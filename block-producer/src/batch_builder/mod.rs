use std::{cmp::min, collections::BTreeMap, sync::Arc, time::Duration};

use async_trait::async_trait;
use miden_objects::{accounts::AccountId, notes::NoteEnvelope, Digest};
use miden_vm::crypto::SimpleSmt;
use tokio::{sync::RwLock, time};

use self::errors::BuildBatchError;
use crate::{block_builder::BlockBuilder, SharedProvenTx, SharedRwVec, SharedTxBatch};

pub mod errors;
#[cfg(test)]
mod tests;

pub(crate) const CREATED_NOTES_SMT_DEPTH: u8 = 13;

/// The created notes tree uses an extra depth to store the 2 components of `NoteEnvelope`.
/// That is, conceptually, notes sit at depth 12; where in reality, depth 12 contains the
/// hash of level 13, where both the `note_hash()` and metadata are stored (one per node).
pub(crate) const MAX_NUM_CREATED_NOTES_PER_BATCH: usize =
    2_usize.pow((CREATED_NOTES_SMT_DEPTH - 1) as u32);

// TRANSACTION BATCH
// ================================================================================================

/// A batch of transactions that share a common proof. For any given account, at most 1 transaction
/// in the batch must be addressing that account.
///
/// Note: Until recursive proofs are available in the Miden VM, we don't include the common proof.
pub struct TransactionBatch {
    updated_accounts: BTreeMap<AccountId, AccountStates>,
    produced_nullifiers: Vec<Digest>,
    created_notes_smt: SimpleSmt,
    /// The notes stored `created_notes_smt`
    created_notes: Vec<NoteEnvelope>,
}

impl TransactionBatch {
    pub fn new(txs: Vec<SharedProvenTx>) -> Result<Self, BuildBatchError> {
        let updated_accounts = txs
            .iter()
            .map(|tx| {
                (
                    tx.account_id(),
                    AccountStates {
                        initial_state: tx.initial_account_hash(),
                        final_state: tx.final_account_hash(),
                    },
                )
            })
            .collect();

        let produced_nullifiers = txs
            .iter()
            .flat_map(|tx| tx.consumed_notes())
            .map(|consumed_note| consumed_note.nullifier())
            .collect();

        let (created_notes, created_notes_smt) = {
            let created_notes: Vec<NoteEnvelope> =
                txs.iter().flat_map(|tx| tx.created_notes()).cloned().collect();

            if created_notes.len() > MAX_NUM_CREATED_NOTES_PER_BATCH {
                return Err(BuildBatchError::TooManyNotesCreated(created_notes.len()));
            }

            (
                created_notes.clone(),
                SimpleSmt::with_contiguous_leaves(
                    CREATED_NOTES_SMT_DEPTH,
                    created_notes.into_iter().flat_map(|note_envelope| {
                        [note_envelope.note_hash().into(), note_envelope.metadata().into()]
                    }),
                )?,
            )
        };

        Ok(Self {
            updated_accounts,
            produced_nullifiers,
            created_notes_smt,
            created_notes,
        })
    }

    /// Returns an iterator over account ids that were modified in the transaction batch, and their
    /// corresponding initial hash
    pub fn account_initial_states(&self) -> impl Iterator<Item = (AccountId, Digest)> + '_ {
        self.updated_accounts
            .iter()
            .map(|(account_id, account_states)| (*account_id, account_states.initial_state))
    }

    /// Returns an iterator over account ids that were modified in the transaction batch, and their
    /// corresponding new hash
    pub fn updated_accounts(&self) -> impl Iterator<Item = (AccountId, Digest)> + '_ {
        self.updated_accounts
            .iter()
            .map(|(account_id, account_states)| (*account_id, account_states.final_state))
    }

    /// Returns the nullifier of all consumed notes
    pub fn produced_nullifiers(&self) -> impl Iterator<Item = Digest> + '_ {
        self.produced_nullifiers.iter().cloned()
    }

    /// Returns the hash of created notes
    pub fn created_notes(&self) -> impl Iterator<Item = &NoteEnvelope> + '_ {
        self.created_notes.iter()
    }

    /// Returns the root of the created notes SMT
    pub fn created_notes_root(&self) -> Digest {
        self.created_notes_smt.root()
    }
}

/// Stores the initial state (before the transaction) and final state (after the transaction) of an
/// account
struct AccountStates {
    initial_state: Digest,
    final_state: Digest,
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
