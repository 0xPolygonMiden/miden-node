//! Abstraction to synchornize state modifications.
//!
//! The [State] provides data access and modifications methods, its main purpose is to ensure that
//! data is atomically written, and that reads are consistent.
use std::mem;

use anyhow::{anyhow, Result};
use miden_crypto::{
    hash::rpo::RpoDigest,
    merkle::{
        MerkleError, MerklePath, Mmr, MmrDelta, MmrPeaks, SimpleSmt, TieredSmt, TieredSmtProof,
    },
    Felt, FieldElement, Word, EMPTY_WORD,
};
use miden_node_proto::{
    block_header,
    conversion::nullifier_value_to_blocknum,
    digest::Digest,
    error::ParseError,
    note::Note,
    requests,
    responses::{
        AccountBlockInputRecord, AccountTransactionInputRecord, NullifierTransactionInputRecord,
    },
};
use miden_objects::{
    notes::{NoteMetadata, NOTE_LEAF_DEPTH},
    BlockHeader,
};
use tokio::{
    sync::{oneshot, Mutex, RwLock},
    time::Instant,
};
use tracing::{info, instrument, span, Level};

use crate::{
    db::{Db, StateSyncUpdate},
    errors::StateError,
    types::{AccountId, BlockNumber},
};

// CONSTANTS
// ================================================================================================

const ACCOUNT_DB_DEPTH: u8 = 64;

// STRUCTURES
// ================================================================================================

/// Container for state that needs to be updated atomically.
struct InnerState {
    nullifier_tree: TieredSmt,
    chain_mmr: Mmr,
    account_tree: SimpleSmt,
}

/// The rollup state
pub struct State {
    db: Db,

    /// Read-write lock used to prevent writing to a structure while it is being used.
    ///
    /// The lock is writer-preferring, meaning the writer won't be starved.
    inner: RwLock<InnerState>,

    /// To allow readers to access the tree data while an update in being performed, and prevent
    /// TOCTOU issues, there must be no concurrent writers. This locks to serialize the writers.
    writer: Mutex<()>,
}

pub struct AccountStateWithProof {
    account_id: AccountId,
    account_hash: Word,
    merkle_path: MerklePath,
}

impl From<AccountStateWithProof> for AccountBlockInputRecord {
    fn from(value: AccountStateWithProof) -> Self {
        Self {
            account_id: Some(value.account_id.into()),
            account_hash: Some(value.account_hash.into()),
            proof: Some(value.merkle_path.into()),
        }
    }
}

pub struct AccountState {
    account_id: AccountId,
    account_hash: Word,
}

impl From<AccountState> for AccountTransactionInputRecord {
    fn from(value: AccountState) -> Self {
        Self {
            account_id: Some(value.account_id.into()),
            account_hash: Some(value.account_hash.into()),
        }
    }
}

impl TryFrom<requests::AccountUpdate> for AccountState {
    type Error = StateError;

    fn try_from(value: requests::AccountUpdate) -> Result<Self, Self::Error> {
        Ok(Self {
            account_id: value.account_id.ok_or(StateError::MissingAccountId)?.into(),
            account_hash: value
                .account_hash
                .ok_or(StateError::MissingAccountHash)?
                .try_into()
                .map_err(StateError::DigestError)?,
        })
    }
}

impl TryFrom<&requests::AccountUpdate> for AccountState {
    type Error = StateError;

    fn try_from(value: &requests::AccountUpdate) -> Result<Self, Self::Error> {
        value.clone().try_into()
    }
}

pub struct NullifierStateForTransactionInput {
    nullifier: RpoDigest,
    block_num: u32,
}

impl From<NullifierStateForTransactionInput> for NullifierTransactionInputRecord {
    fn from(value: NullifierStateForTransactionInput) -> Self {
        Self {
            nullifier: Some(value.nullifier.into()),
            block_num: value.block_num,
        }
    }
}

impl State {
    pub async fn load(mut db: Db) -> Result<Self, anyhow::Error> {
        let nullifier_tree = load_nullifier_tree(&mut db).await?;
        let chain_mmr = load_mmr(&mut db).await?;
        let account_tree = load_accounts(&mut db).await?;

        let inner = RwLock::new(InnerState {
            nullifier_tree,
            chain_mmr,
            account_tree,
        });

        let writer = Mutex::new(());
        Ok(Self { db, inner, writer })
    }

    /// Apply changes of a new block to the DB and in-memory data structures.
    ///
    /// ## Note on state consistency
    ///
    /// The server contains in-memory representations of the existing trees, the in-memory
    /// representation must be kept consistent with the commited data, this is necessary so to
    /// provide consistent results for all endpoints. In order to achieve consistency, the
    /// following steps are used:
    ///
    /// - the request data is validated, prior to starting any modifications
    /// - a transaction is open in the DB and the writes are started
    ///  - while the transaction is not commited, concurrent reads are allowed, both the DB and
    ///  the in-memory representations, which are consistent at this stage.
    /// - prior to commiting the changes to the DB, an exclusive lock to the in-memory data is
    /// acquired, preventing concurrent reads to the in-memory data, since that will be
    /// out-of-sync w.r.t. the DB.
    /// - the DB transaction is commited, and requests that read only from the DB can proceed to
    /// use the fresh data
    /// - the in-memory structures are updated, and the lock is released
    pub async fn apply_block(
        &self,
        block_header: block_header::BlockHeader,
        nullifiers: &[RpoDigest],
        accounts: &[(AccountId, Digest)],
        notes: &[Note],
    ) -> Result<(), anyhow::Error> {
        let _ = self.writer.try_lock().map_err(|_| StateError::ConcurrentWrite)?;

        // ensures the right block header is being processed
        let prev_block_msg = self
            .db
            .select_block_header_by_block_num(None)
            .await?
            .ok_or(StateError::DbBlockHeaderEmpty)?;
        let block_num = (prev_block_msg.block_num as u32) + 1;
        let prev_block: BlockHeader = prev_block_msg.try_into()?;
        let prev_hash =
            block_header.prev_hash.clone().ok_or(StateError::MissingPrevHash)?.try_into()?;
        if prev_hash != prev_block.hash() {
            return Err(StateError::NewBlockInvalidPrevHash.into());
        }

        // scope to read in-memory data, validate the request, and compute intermediary values
        let (account_tree, chain_mmr, nullifier_tree) = {
            let inner = self.inner.read().await;

            let span = span!(Level::INFO, "updating in-memory data structures");
            let guard = span.enter();

            // nullifiers can be produced only once
            let duplicate_nullifiers: Vec<_> = nullifiers
                .iter()
                .filter(|&&n| inner.nullifier_tree.get_value(n) != EMPTY_WORD)
                .cloned()
                .collect();
            if !duplicate_nullifiers.is_empty() {
                return Err(StateError::DuplicatedNullifiers(duplicate_nullifiers).into());
            }

            // update the in-memory datastructures and compute the new block header. Important, the
            // structures are not yet commited
            let mut chain_mmr = inner.chain_mmr.clone();
            chain_mmr.add(prev_hash);
            let peaks = chain_mmr.peaks(chain_mmr.forest())?;

            if RpoDigest::from(peaks.hash_peaks())
                != block_header
                    .clone()
                    .chain_root
                    .ok_or(StateError::MissingChainRoot)?
                    .try_into()?
            {
                return Err(StateError::InvalidChainRoot.into());
            }

            let mut nullifier_tree = inner.nullifier_tree.clone();
            let nullifier_data = block_to_nullifier_data(block_num);
            for nullifier in nullifiers {
                nullifier_tree.insert(*nullifier, nullifier_data);
            }

            if nullifier_tree.root()
                != block_header
                    .nullifier_root
                    .clone()
                    .ok_or(StateError::MissingNullifierRoot)?
                    .try_into()?
            {
                return Err(StateError::InvalidNullifierRoot.into());
            }

            let mut account_tree = inner.account_tree.clone();
            for (account_id, account_hash) in accounts {
                account_tree.update_leaf(*account_id, account_hash.try_into()?)?;
            }

            if account_tree.root()
                != block_header
                    .account_root
                    .clone()
                    .ok_or(StateError::MissingAccountRoot)?
                    .try_into()?
            {
                return Err(StateError::InvalidAccountRoot.into());
            }

            let note_tree = build_notes_tree(notes)?;
            if note_tree.root()
                != block_header.note_root.clone().ok_or(StateError::MissingNoteRoot)?.try_into()?
            {
                return Err(StateError::InvalidNoteRoot.into());
            }

            drop(guard);

            (account_tree, chain_mmr, nullifier_tree)
        };

        // signals the transaction is ready to be commited, and the write lock can be acquired
        let (allow_acquire, acquired_allowed) = oneshot::channel::<()>();
        // signals the write lock has been acquired, and the transaction can be commited
        let (inform_acquire_done, acquire_done) = oneshot::channel::<()>();

        self.db
            .apply_block(allow_acquire, acquire_done, block_header, notes, nullifiers, accounts)
            .await?;

        acquired_allowed.await?;

        // scope to update the in-memory data
        {
            let mut inner = self.inner.write().await;
            let _ = inform_acquire_done.send(());

            let _ = mem::replace(&mut inner.chain_mmr, chain_mmr);
            let _ = mem::replace(&mut inner.nullifier_tree, nullifier_tree);
            let _ = mem::replace(&mut inner.account_tree, account_tree);
        }

        Ok(())
    }

    pub async fn get_block_header(
        &self,
        block_num: Option<BlockNumber>,
    ) -> Result<Option<block_header::BlockHeader>, anyhow::Error> {
        self.db.select_block_header_by_block_num(block_num).await
    }

    pub async fn check_nullifiers(
        &self,
        nullifiers: &[RpoDigest],
    ) -> Vec<TieredSmtProof> {
        let inner = self.inner.read().await;
        nullifiers.iter().map(|n| inner.nullifier_tree.prove(*n)).collect()
    }

    pub async fn sync_state(
        &self,
        block_num: BlockNumber,
        account_ids: &[AccountId],
        note_tag_prefixes: &[u32],
        nullifier_prefixes: &[u32],
    ) -> Result<(StateSyncUpdate, MmrDelta, MerklePath), anyhow::Error> {
        let inner = self.inner.read().await;

        let state_sync = self
            .db
            .get_state_sync(block_num, account_ids, note_tag_prefixes, nullifier_prefixes)
            .await?;

        let (delta, path) = {
            let delta = inner
                .chain_mmr
                .get_delta(block_num as usize, state_sync.block_header.block_num as usize)?;

            let proof = inner.chain_mmr.open(
                state_sync.block_header.block_num as usize,
                state_sync.block_header.block_num as usize,
            )?;

            (delta, proof.merkle_path)
        };

        Ok((state_sync, delta, path))
    }

    /// Returns data needed by the block producer to construct and prove the next block.
    pub async fn get_block_inputs(
        &self,
        account_ids: &[AccountId],
        _nullifiers: &[RpoDigest],
    ) -> Result<(block_header::BlockHeader, MmrPeaks, Vec<AccountStateWithProof>), anyhow::Error>
    {
        let inner = self.inner.read().await;

        let latest = self
            .db
            .select_block_header_by_block_num(None)
            .await?
            .ok_or(anyhow!("Database is empty"))?;
        let accumulator = inner.chain_mmr.peaks(latest.block_num as usize)?;
        let account_states = account_ids
            .iter()
            .cloned()
            .map(|account_id| {
                let account_hash = inner.account_tree.get_leaf(account_id)?;
                let merkle_path = inner.account_tree.get_leaf_path(account_id)?;
                Ok(AccountStateWithProof {
                    account_id,
                    account_hash,
                    merkle_path,
                })
            })
            .collect::<Result<Vec<AccountStateWithProof>, MerkleError>>()?;

        // TODO: add nullifiers
        Ok((latest, accumulator, account_states))
    }

    pub async fn get_transaction_inputs(
        &self,
        account_ids: &[AccountId],
        nullifiers: &[RpoDigest],
    ) -> Result<(Vec<AccountState>, Vec<NullifierStateForTransactionInput>), anyhow::Error> {
        let inner = self.inner.read().await;

        let accounts: Vec<_> = account_ids
            .iter()
            .cloned()
            .map(|id| {
                Ok(AccountState {
                    account_id: id,
                    account_hash: inner.account_tree.get_leaf(id)?,
                })
            })
            .collect::<Result<Vec<AccountState>, MerkleError>>()?;

        let nullifier_blocks = nullifiers
            .iter()
            .cloned()
            .map(|nullifier| {
                let value = inner.nullifier_tree.get_value(nullifier);
                let block_num = nullifier_value_to_blocknum(value);

                NullifierStateForTransactionInput {
                    nullifier,
                    block_num,
                }
            })
            .collect();

        Ok((accounts, nullifier_blocks))
    }
}

// UTILITIES
// ================================================================================================

/// Returns the nullifier's block number given its leaf value in the TSMT.
fn block_to_nullifier_data(block: BlockNumber) -> Word {
    [Felt::new(block as u64), Felt::ZERO, Felt::ZERO, Felt::ZERO]
}

// /// Returns the block number encoded as a leaf value to be used in the TSMT.
// fn nullifier_data_to_block(block: Word) -> BlockNumber {
//     block[0].as_int() as BlockNumber
// }

/// Creates a [SimpleSmt] tree from the `notes`.
pub fn build_notes_tree(notes: &[Note]) -> Result<SimpleSmt, anyhow::Error> {
    // TODO: create SimpleSmt without this allocation
    let mut entries: Vec<(u64, Word)> = Vec::with_capacity(notes.len() * 2);

    for (index, note) in notes.iter().enumerate() {
        let note_hash = note.note_hash.clone().ok_or(StateError::MissingNoteHash)?;
        let account_id = note.sender.try_into().or(Err(StateError::InvalidAccountId))?;
        let note_metadata = NoteMetadata::new(account_id, note.tag.into(), note.num_assets.into());
        let index = (index as u64) * 2;
        entries.push((index, note_hash.try_into()?));
        entries.push((index + 1, note_metadata.into()));
    }

    Ok(SimpleSmt::with_leaves(NOTE_LEAF_DEPTH, entries)?)
}

#[instrument(skip(db))]
async fn load_nullifier_tree(db: &mut Db) -> Result<TieredSmt> {
    let nullifiers = db.select_nullifiers().await?;
    let len = nullifiers.len();
    let leaves = nullifiers
        .into_iter()
        .map(|(nullifier, block)| (nullifier, block_to_nullifier_data(block)));

    let now = Instant::now();
    let nullifier_tree = TieredSmt::with_entries(leaves)?;
    let elapsed = now.elapsed().as_secs();

    info!(num_of_leaves = len, tree_construction = elapsed, "Loaded nullifier tree");
    Ok(nullifier_tree)
}

#[instrument(skip(db))]
async fn load_mmr(db: &mut Db) -> Result<Mmr> {
    let block_hashes: Result<Vec<RpoDigest>, ParseError> = db
        .select_block_headers()
        .await?
        .into_iter()
        .map(|b| b.try_into().map(|b: BlockHeader| b.hash()))
        .collect();

    let mmr: Mmr = block_hashes?.into();
    Ok(mmr)
}

#[instrument(skip(db))]
async fn load_accounts(db: &mut Db) -> Result<SimpleSmt> {
    let account_data: Result<Vec<(u64, Word)>> = db
        .select_account_hashes()
        .await?
        .into_iter()
        .map(|(id, account_hash)| Ok((id, account_hash.try_into()?)))
        .collect();

    let smt = SimpleSmt::with_leaves(ACCOUNT_DB_DEPTH, account_data?)?;

    Ok(smt)
}
