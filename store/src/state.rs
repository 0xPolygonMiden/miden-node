//! Abstraction to synchornize state modifications.
//!
//! The [State] provides data access and modifications methods, its main purpose is to ensure that
//! data is atomically written, and that reads are consistent.
use std::{mem, sync::Arc};

use anyhow::{anyhow, bail, Result};
use miden_crypto::{
    hash::rpo::RpoDigest,
    merkle::{
        LeafIndex, MerkleError, MerklePath, Mmr, MmrDelta, MmrPeaks, SimpleSmt, TieredSmt,
        TieredSmtProof, ValuePath,
    },
    Felt, FieldElement, Word, EMPTY_WORD,
};
use miden_node_proto::{
    account::AccountInfo,
    block_header,
    conversion::nullifier_value_to_blocknum,
    digest::Digest,
    error::ParseError,
    note::{Note, NoteCreated},
    requests::AccountUpdate,
    responses::{
        AccountBlockInputRecord, AccountTransactionInputRecord, NullifierTransactionInputRecord,
    },
};
use miden_objects::{
    notes::{NoteMetadata, NOTE_LEAF_DEPTH},
    BlockHeader, ACCOUNT_TREE_DEPTH,
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
    COMPONENT,
};

// STRUCTURES
// ================================================================================================

/// Container for state that needs to be updated atomically.
struct InnerState {
    nullifier_tree: TieredSmt,
    chain_mmr: Mmr,
    account_tree: SimpleSmt<ACCOUNT_TREE_DEPTH>,
}

/// The rollup state
pub struct State {
    db: Arc<Db>,

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

impl TryFrom<AccountUpdate> for AccountState {
    type Error = StateError;

    fn try_from(value: AccountUpdate) -> Result<Self, Self::Error> {
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

impl TryFrom<&AccountUpdate> for AccountState {
    type Error = StateError;

    fn try_from(value: &AccountUpdate) -> Result<Self, Self::Error> {
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
    /// Loads the state from the `db`.
    #[instrument(skip(db))]
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
        let db = Arc::new(db);
        Ok(Self { db, inner, writer })
    }

    /// Apply changes of a new block to the DB and in-memory data structures.
    ///
    /// ## Note on state consistency
    ///
    /// The server contains in-memory representations of the existing trees, the in-memory
    /// representation must be kept consistent with the committed data, this is necessary so to
    /// provide consistent results for all endpoints. In order to achieve consistency, the
    /// following steps are used:
    ///
    /// - the request data is validated, prior to starting any modifications.
    /// - a transaction is open in the DB and the writes are started.
    /// - while the transaction is not committed, concurrent reads are allowed, both the DB and
    ///   the in-memory representations, which are consistent at this stage.
    /// - prior to committing the changes to the DB, an exclusive lock to the in-memory data is
    ///   acquired, preventing concurrent reads to the in-memory data, since that will be
    ///   out-of-sync w.r.t. the DB.
    /// - the DB transaction is committed, and requests that read only from the DB can proceed to
    ///   use the fresh data.
    /// - the in-memory structures are updated, and the lock is released.
    pub async fn apply_block(
        &self,
        block_header: block_header::BlockHeader,
        nullifiers: Vec<RpoDigest>,
        accounts: Vec<(AccountId, Digest)>,
        notes: Vec<NoteCreated>,
    ) -> Result<(), anyhow::Error> {
        let _ = self.writer.try_lock().map_err(|_| StateError::ConcurrentWrite)?;

        let new_block: BlockHeader = block_header.clone().try_into()?;

        // ensures the right block header is being processed
        let prev_block: BlockHeader = self
            .db
            .select_block_header_by_block_num(None)
            .await?
            .ok_or(StateError::DbBlockHeaderEmpty)?
            .try_into()?;

        if new_block.block_num() != prev_block.block_num() + 1 {
            return Err(StateError::NewBlockInvalidBlockNum.into());
        }
        if new_block.prev_hash() != prev_block.hash() {
            return Err(StateError::NewBlockInvalidPrevHash.into());
        }

        // scope to read in-memory data, validate the request, and compute intermediary values
        let (account_tree, chain_mmr, nullifier_tree, notes) = {
            let inner = self.inner.read().await;

            let span = span!(Level::INFO, COMPONENT, "updating in-memory data structures");
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

            // update the in-memory data structures and compute the new block header. Important, the
            // structures are not yet committed

            // update chain MMR
            let chain_mmr = {
                let mut chain_mmr = inner.chain_mmr.clone();

                // new_block.chain_root must be equal to the chain MMR root prior to the update
                let peaks = chain_mmr.peaks(chain_mmr.forest())?;
                if peaks.hash_peaks() != new_block.chain_root() {
                    return Err(StateError::NewBlockInvalidChainRoot.into());
                }

                chain_mmr.add(new_block.hash());
                chain_mmr
            };

            // update nullifier tree
            let nullifier_tree = {
                let mut nullifier_tree = inner.nullifier_tree.clone();
                let nullifier_data = block_to_nullifier_data(new_block.block_num());
                for nullifier in nullifiers.iter() {
                    nullifier_tree.insert(*nullifier, nullifier_data);
                }

                // FIXME: Re-add when nullifiers start getting updated
                // if nullifier_tree.root() != new_block.nullifier_root() {
                //     return Err(StateError::NewBlockInvalidNullifierRoot.into());
                // }
                nullifier_tree
            };

            // update account tree
            let mut account_tree = inner.account_tree.clone();
            for (account_id, account_hash) in accounts.iter() {
                account_tree
                    .insert(LeafIndex::new_max_depth(*account_id), account_hash.try_into()?);
            }

            if account_tree.root() != new_block.account_root() {
                return Err(StateError::NewBlockInvalidAccountRoot.into());
            }

            // build notes tree
            let note_tree = build_notes_tree(&notes)?;
            if note_tree.root() != new_block.note_root() {
                return Err(StateError::NewBlockInvalidNoteRoot.into());
            }

            drop(guard);

            let notes = notes
                .iter()
                .map(|note| {
                    // Safety: This should never happen, the note_tree is created directly form
                    // this list of notes
                    let leaf_index: LeafIndex<NOTE_LEAF_DEPTH> =
                        LeafIndex::new(note.note_index as u64).map_err(|_| {
                            anyhow!("Internal server error, unable to created proof for note")
                        })?;

                    let merkle_path = note_tree.open(&leaf_index).path;

                    Ok(Note {
                        block_num: new_block.block_num(),
                        note_hash: note.note_hash.clone(),
                        sender: note.sender,
                        note_index: note.note_index,
                        tag: note.tag,
                        merkle_path: Some(merkle_path.into()),
                    })
                })
                .collect::<Result<Vec<_>, anyhow::Error>>()?;

            (account_tree, chain_mmr, nullifier_tree, notes)
        };

        // signals the transaction is ready to be committed, and the write lock can be acquired
        let (allow_acquire, acquired_allowed) = oneshot::channel::<()>();
        // signals the write lock has been acquired, and the transaction can be committed
        let (inform_acquire_done, acquire_done) = oneshot::channel::<()>();

        // The DB and in-memory state updates need to be synchronized and are partially
        // overlapping. Namely, the DB transaction only proceeds after this task acquires the
        // in-memory write lock. This requires the DB update to run concurrently, so a new task is
        // spawned.
        let db = self.db.clone();
        tokio::spawn(async move {
            db.apply_block(allow_acquire, acquire_done, block_header, notes, nullifiers, accounts)
                .await
        });

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

    /// Queries a [BlockHeader] from the database.
    ///
    /// If [None] is given as the value of `block_num`, the latest [BlockHeader] is returned.
    pub async fn get_block_header(
        &self,
        block_num: Option<BlockNumber>,
    ) -> Result<Option<block_header::BlockHeader>, anyhow::Error> {
        self.db.select_block_header_by_block_num(block_num).await
    }

    /// Generates membership proofs for each one of the `nullifiers` against the latest nullifier
    /// tree.
    ///
    /// Note: these proofs are invalidated once the nullifier tree is modified, i.e. on a new block.
    pub async fn check_nullifiers(
        &self,
        nullifiers: &[RpoDigest],
    ) -> Vec<TieredSmtProof> {
        let inner = self.inner.read().await;
        nullifiers.iter().map(|n| inner.nullifier_tree.prove(*n)).collect()
    }

    /// Loads data to synchronize a client.
    ///
    /// The client's request contains a list of tag prefixes, this method will return the first
    /// block with a matching tag, or the chain tip. All the other values are filter based on this
    /// block range.
    ///
    /// # Arguments
    ///
    /// - `block_num`: The last block *know* by the client, updates start from the next block.
    /// - `account_ids`: Include the account's hash if their _last change_ was in the result's block
    ///   range.
    /// - `note_tag_prefixes`: Only the 16 high bits of the tags the client is interested in, result
    ///   will include notes with matching prefixes, the first block with a matching note determines
    ///   the block range.
    /// - `nullifier_prefixes`: Only the 16 high bits of the nullifiers the client is intersted in,
    ///   results will cinlude nullifiers matching prefixes produced in the given block range.
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

            let proof = inner
                .chain_mmr
                .open(block_num as usize, state_sync.block_header.block_num as usize)?;

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
            .ok_or(StateError::DbBlockHeaderEmpty)?;

        // sanity check
        if inner.chain_mmr.forest() != latest.block_num as usize + 1 {
            bail!(
                "chain MMR forest expected to be 1 less than latest header's block num. Chain MMR forest: {}, block num: {}",
                inner.chain_mmr.forest(),
                latest.block_num
            );
        }

        // using current block number gets us the peaks of the chain MMR as of one block ago;
        // this is done so that latest.chain_root matches the returned peaks
        let peaks = inner.chain_mmr.peaks(latest.block_num as usize)?;
        let account_states = account_ids
            .iter()
            .cloned()
            .map(|account_id| {
                let ValuePath {
                    value: account_hash,
                    path: merkle_path,
                } = inner.account_tree.open(&LeafIndex::new_max_depth(account_id));
                Ok(AccountStateWithProof {
                    account_id,
                    account_hash: account_hash.into(),
                    merkle_path,
                })
            })
            .collect::<Result<Vec<AccountStateWithProof>, MerkleError>>()?;

        // TODO: add nullifiers
        Ok((latest, peaks, account_states))
    }

    /// Returns data needed by the block producer to verify transactions validity.
    pub async fn get_transaction_inputs(
        &self,
        account_id: AccountId,
        nullifiers: &[RpoDigest],
    ) -> Result<(AccountState, Vec<NullifierStateForTransactionInput>), anyhow::Error> {
        let inner = self.inner.read().await;

        let account = AccountState {
            account_id,
            account_hash: inner
                .account_tree
                .open(&LeafIndex::new_max_depth(account_id))
                .value
                .into(),
        };

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

        Ok((account, nullifier_blocks))
    }

    /// Lists all known nullifiers with their inclusion blocks, intended for testing.
    pub async fn list_nullifiers(&self) -> Result<Vec<(RpoDigest, u32)>, anyhow::Error> {
        let nullifiers = self.db.select_nullifiers().await?;
        Ok(nullifiers)
    }

    /// Lists all known accounts, with their ids, latest state hash, and block at which the account was last
    /// modified, intended for testing.
    pub async fn list_accounts(&self) -> Result<Vec<AccountInfo>, anyhow::Error> {
        let accounts = self.db.select_accounts().await?;
        Ok(accounts)
    }

    /// Lists all known notes, intended for testing.
    pub async fn list_notes(&self) -> Result<Vec<Note>, anyhow::Error> {
        let notes = self.db.select_notes().await?;
        Ok(notes)
    }
}

// UTILITIES
// ================================================================================================

/// Returns the nullifier's block number given its leaf value in the TSMT.
fn block_to_nullifier_data(block: BlockNumber) -> Word {
    [Felt::new(block as u64), Felt::ZERO, Felt::ZERO, Felt::ZERO]
}

/// Creates a [SimpleSmt] tree from the `notes`.
pub fn build_notes_tree(
    notes: &[NoteCreated]
) -> Result<SimpleSmt<NOTE_LEAF_DEPTH>, anyhow::Error> {
    // TODO: create SimpleSmt without this allocation
    let mut entries: Vec<(u64, Word)> = Vec::with_capacity(notes.len() * 2);

    for note in notes.iter() {
        let note_hash = note.note_hash.clone().ok_or(StateError::MissingNoteHash)?;
        let account_id = note.sender.try_into().or(Err(StateError::InvalidAccountId))?;
        let note_metadata = NoteMetadata::new(account_id, note.tag.into());
        let index = note.note_index as u64;
        entries.push((index, note_hash.try_into()?));
        entries.push((index + 1, note_metadata.into()));
    }

    Ok(SimpleSmt::with_leaves(entries)?)
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

    info!(
        num_of_leaves = len,
        tree_construction = elapsed,
        COMPONENT,
        "Loaded nullifier tree"
    );
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
async fn load_accounts(db: &mut Db) -> Result<SimpleSmt<ACCOUNT_TREE_DEPTH>> {
    let account_data: Result<Vec<(u64, Word)>> = db
        .select_account_hashes()
        .await?
        .into_iter()
        .map(|(id, account_hash)| Ok((id, account_hash.try_into()?)))
        .collect();

    let smt = SimpleSmt::with_leaves(account_data?)?;

    Ok(smt)
}
