use std::{cmp::min, sync::Arc, time::Duration};

use async_trait::async_trait;
use itertools::Itertools;
use miden_objects::{accounts::AccountId, Digest};
use miden_vm::crypto::SimpleSmt;
use tokio::{sync::RwLock, time};

use crate::{block_builder::BlockBuilder, SharedProvenTx, SharedRwVec, SharedTxBatch};

use self::errors::BuildBatchError;

pub mod errors;
#[cfg(test)]
mod tests;

pub(crate) const CREATED_NOTES_SMT_DEPTH: u8 = 12;
const MAX_NUM_CREATED_NOTES_PER_BATCH: usize = 2usize.pow(CREATED_NOTES_SMT_DEPTH as u32);

// TRANSACTION BATCH
// ================================================================================================

/// A batch of transactions that share a common proof. For any given account, at most 1 transaction
/// in the batch must be addressing that account.
///
/// Note: Until recursive proofs are available in the Miden VM, we don't include the common proof.
pub struct TransactionBatch {
    account_initial_states: Vec<(AccountId, Digest)>,
    updated_accounts: Vec<(AccountId, Digest)>,
    consumed_notes_script_roots: Vec<Digest>,
    produced_nullifiers: Vec<Digest>,
    created_notes_smt: SimpleSmt,
}

impl TransactionBatch {
    pub fn new(txs: Vec<SharedProvenTx>) -> Result<Self, BuildBatchError> {
        let account_initial_states =
            txs.iter().map(|tx| (tx.account_id(), tx.initial_account_hash())).collect();
        let updated_accounts =
            txs.iter().map(|tx| (tx.account_id(), tx.final_account_hash())).collect();

        let consumed_notes_script_roots = {
            let mut script_roots: Vec<Digest> = txs
                .iter()
                .flat_map(|tx| tx.consumed_notes())
                .map(|consumed_note| consumed_note.script_root())
                .collect();

            script_roots.sort();

            // Removes duplicates in consecutive items
            script_roots.into_iter().dedup().collect()
        };
        let produced_nullifiers = txs
            .iter()
            .flat_map(|tx| tx.consumed_notes())
            .map(|consumed_note| consumed_note.nullifier())
            .collect();

        let created_notes_smt = {
            let created_notes: Vec<_> = txs
                .iter()
                .flat_map(|tx| tx.created_notes())
                .map(|note_envelope| note_envelope.note_hash())
                .collect();

            if created_notes.len() > MAX_NUM_CREATED_NOTES_PER_BATCH {
                return Err(BuildBatchError::TooManyNotesCreated(created_notes.len()));
            }

            SimpleSmt::with_leaves(
                CREATED_NOTES_SMT_DEPTH,
                created_notes.into_iter().enumerate().map(|(idx, note_hash)| {
                    (
                        idx.try_into().expect("already checked not for too many notes"),
                        note_hash.into(),
                    )
                }),
            )?
        };

        Ok(Self {
            account_initial_states,
            updated_accounts,
            consumed_notes_script_roots,
            produced_nullifiers,
            created_notes_smt,
        })
    }

    /// Returns an iterator over account ids that were modified in the transaction batch, and their
    /// corresponding initial hash
    pub fn account_initial_states(&self) -> impl Iterator<Item = (AccountId, Digest)> + '_ {
        self.account_initial_states.iter().cloned()
    }

    /// Returns an iterator over account ids that were modified in the transaction batch, and their
    /// corresponding new hash
    pub fn updated_accounts(&self) -> impl Iterator<Item = (AccountId, Digest)> + '_ {
        self.updated_accounts.iter().cloned()
    }

    /// Returns the script root of all consumed notes
    pub fn consumed_notes_script_roots(&self) -> impl Iterator<Item = Digest> + '_ {
        self.consumed_notes_script_roots.iter().cloned()
    }

    /// Returns the nullifier of all consumed notes
    pub fn produced_nullifiers(&self) -> impl Iterator<Item = Digest> + '_ {
        self.produced_nullifiers.iter().cloned()
    }

    /// Returns the hash of created notes
    pub fn created_notes(&self) -> impl Iterator<Item = Digest> + '_ {
        self.created_notes_smt.leaves().map(|(_key, leaf)| leaf.into())
    }

    /// Returns the root of the created notes SMT
    pub fn created_notes_root(&self) -> Digest {
        self.created_notes_smt.root()
    }
}

// BATCH BUILDER
// ================================================================================================

#[async_trait]
pub trait BatchBuilder: Send + Sync + 'static {
    async fn build_batch(
        &self,
        txs: Vec<SharedProvenTx>,
    ) -> Result<(), BuildBatchError>;
}

pub struct DefaultBatchBuilderOptions {
    /// The frequency at which blocks are created
    pub block_frequency: Duration,

    /// Maximum number of batches in any given block
    pub max_batches_per_block: usize,
}

pub struct DefaultBatchBuilder<BB> {
    /// Batches ready to be included in a block
    ready_batches: SharedRwVec<SharedTxBatch>,

    block_builder: Arc<BB>,

    options: DefaultBatchBuilderOptions,
}

impl<BB> DefaultBatchBuilder<BB>
where
    BB: BlockBuilder,
{
    pub fn new(
        block_builder: Arc<BB>,
        options: DefaultBatchBuilderOptions,
    ) -> Self {
        Self {
            ready_batches: Arc::new(RwLock::new(Vec::new())),
            block_builder,
            options,
        }
    }

    pub async fn run(self) {
        let mut interval = time::interval(self.options.block_frequency);

        loop {
            interval.tick().await;
            self.try_build_block().await;
        }
    }

    /// Note that we call `build_block()` regardless of whether the `ready_batches` queue is empty.
    /// A call to an empty `build_block()` indicates that an empty block should be created.
    async fn try_build_block(&self) {
        let mut batches_in_block: Vec<SharedTxBatch> = {
            let mut locked_ready_batches = self.ready_batches.write().await;

            let num_batches_in_block =
                min(self.options.max_batches_per_block, locked_ready_batches.len());

            locked_ready_batches.drain(..num_batches_in_block).collect()
        };

        match self.block_builder.build_block(batches_in_block.clone()).await {
            Ok(_) => {
                // block successfully built, do nothing
            },
            Err(_) => {
                // Block building failed; add back the batches at the end of the queue
                self.ready_batches.write().await.append(&mut batches_in_block);
            },
        }
    }
}

#[async_trait]
impl<BB> BatchBuilder for DefaultBatchBuilder<BB>
where
    BB: BlockBuilder,
{
    async fn build_batch(
        &self,
        txs: Vec<SharedProvenTx>,
    ) -> Result<(), BuildBatchError> {
        let batch = Arc::new(TransactionBatch::new(txs)?);
        self.ready_batches.write().await.push(batch);

        Ok(())
    }
}
