//! Abstraction to synchornize state modifications.
//!
//! The [State] provides data access and modifications methods, its main purpose is to ensure that
//! data is atomically written, and that reads are consistent.
use crate::{
    db::{Db, StateSyncUpdate},
    types::{AccountId, BlockNumber},
};
use anyhow::{anyhow, Result};
use miden_crypto::{
    hash::rpo::RpoDigest,
    merkle::{
        MerkleError, MerklePath, Mmr, MmrDelta, MmrPeaks, SimpleSmt, TieredSmt, TieredSmtProof,
    },
    Felt, FieldElement, Word,
};
use miden_node_proto::{
    block_header::BlockHeader,
    conversion::nullifier_value_to_blocknum,
    error::ParseError,
    responses::{
        AccountBlockInputRecord, AccountTransactionInputRecord, NullifierTransactionInputRecord,
    },
};
use tokio::{sync::RwLock, time::Instant};
use tracing::{info, instrument};

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
    inner: RwLock<InnerState>,
}

pub struct AccountStateForBlockInput {
    account_id: AccountId,
    account_hash: Word,
    merkle_path: MerklePath,
}

impl From<AccountStateForBlockInput> for AccountBlockInputRecord {
    fn from(value: AccountStateForBlockInput) -> Self {
        Self {
            account_id: Some(value.account_id.into()),
            account_hash: Some(value.account_hash.into()),
            proof: Some(value.merkle_path.into()),
        }
    }
}

pub struct AccountStateForTransactionInput {
    account_id: AccountId,
    account_hash: Word,
}

impl From<AccountStateForTransactionInput> for AccountTransactionInputRecord {
    fn from(value: AccountStateForTransactionInput) -> Self {
        Self {
            account_id: Some(value.account_id.into()),
            account_hash: Some(value.account_hash.into()),
        }
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

        Ok(Self { db, inner })
    }

    pub async fn get_block_header(
        &self,
        block_num: Option<BlockNumber>,
    ) -> Result<Option<BlockHeader>, anyhow::Error> {
        self.db.get_block_header(block_num).await
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
    ) -> Result<(BlockHeader, MmrPeaks, Vec<AccountStateForBlockInput>), anyhow::Error> {
        let inner = self.inner.read().await;

        let latest = self.db.get_block_header(None).await?.ok_or(anyhow!("Database is empty"))?;
        let accumulator = inner.chain_mmr.peaks(latest.block_num as usize)?;
        let account_states = account_ids
            .iter()
            .cloned()
            .map(|account_id| {
                let account_hash = inner.account_tree.get_leaf(account_id)?;
                let merkle_path = inner.account_tree.get_leaf_path(account_id)?;
                Ok(AccountStateForBlockInput {
                    account_id,
                    account_hash,
                    merkle_path,
                })
            })
            .collect::<Result<Vec<AccountStateForBlockInput>, MerkleError>>()?;

        // TODO: add nullifiers
        Ok((latest, accumulator, account_states))
    }

    pub async fn get_transaction_inputs(
        &self,
        account_ids: &[AccountId],
        nullifiers: &[RpoDigest],
    ) -> Result<
        (Vec<AccountStateForTransactionInput>, Vec<NullifierStateForTransactionInput>),
        anyhow::Error,
    > {
        let inner = self.inner.read().await;

        let accounts: Vec<_> = account_ids
            .iter()
            .cloned()
            .map(|id| {
                Ok(AccountStateForTransactionInput {
                    account_id: id,
                    account_hash: inner.account_tree.get_leaf(id)?,
                })
            })
            .collect::<Result<Vec<AccountStateForTransactionInput>, MerkleError>>()?;

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

#[instrument(skip(db))]
async fn load_nullifier_tree(db: &mut Db) -> Result<TieredSmt> {
    let nullifiers = db.get_nullifiers().await?;
    let len = nullifiers.len();
    let leaves = nullifiers.into_iter().map(|(nullifier, block)| {
        (nullifier, [Felt::new(block as u64), Felt::ZERO, Felt::ZERO, Felt::ZERO])
    });

    let now = Instant::now();
    let nullifier_tree = TieredSmt::with_entries(leaves)?;
    let elapsed = now.elapsed().as_secs();

    info!(num_of_leaves = len, tree_construction = elapsed, "Loaded nullifier tree");
    Ok(nullifier_tree)
}

#[instrument(skip(db))]
async fn load_mmr(db: &mut Db) -> Result<Mmr> {
    use miden_objects::BlockHeader;

    let block_hashes: Result<Vec<RpoDigest>, ParseError> = db
        .get_block_headers()
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
        .get_account_hashes()
        .await?
        .into_iter()
        .map(|(id, account_hash)| Ok((id, account_hash.try_into()?)))
        .collect();

    let smt = SimpleSmt::with_leaves(ACCOUNT_DB_DEPTH, account_data?)?;

    Ok(smt)
}
