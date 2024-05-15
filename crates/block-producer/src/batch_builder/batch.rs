use miden_objects::{
    accounts::AccountId,
    batches::BatchNoteTree,
    block::BlockAccountUpdate,
    crypto::hash::blake::{Blake3Digest, Blake3_256},
    notes::Nullifier,
    transaction::{OutputNote, TxAccountUpdate},
    Digest, MAX_NOTES_PER_BATCH,
};
use tracing::instrument;

use crate::{errors::BuildBatchError, ProvenTransaction};

pub type BatchId = Blake3Digest<32>;

// TRANSACTION BATCH
// ================================================================================================

/// A batch of transactions that share a common proof. For any given account, at most 1 transaction
/// in the batch must be addressing that account (issue: #186).
///
/// Note: Until recursive proofs are available in the Miden VM, we don't include the common proof.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransactionBatch {
    id: BatchId,
    updated_accounts: Vec<TxAccountUpdate>,
    produced_nullifiers: Vec<Nullifier>,
    created_notes_smt: BatchNoteTree,
    created_notes: Vec<OutputNote>,
}

impl TransactionBatch {
    // CONSTRUCTOR
    // --------------------------------------------------------------------------------------------
    /// Returns a new [TransactionBatch] instantiated from the provided vector of proven
    /// transactions.
    ///
    /// # Errors
    /// Returns an error if:
    /// - The number of created notes across all transactions exceeds 4096.
    ///
    /// TODO: enforce limit on the number of created nullifiers.
    #[instrument(target = "miden-block-producer", name = "new_batch", skip_all, err)]
    pub fn new(txs: Vec<ProvenTransaction>) -> Result<Self, BuildBatchError> {
        let id = Self::compute_id(&txs);

        // TODO: we need to handle a possibility that a batch contains multiple transactions against
        //       the same account (e.g., transaction `x` takes account from state `A` to `B` and
        //       transaction `y` takes account from state `B` to `C`). These will need to be merged
        //       into a single "update" `A` to `C`.
        let updated_accounts = txs.iter().map(ProvenTransaction::account_update).cloned().collect();

        let produced_nullifiers =
            txs.iter().flat_map(|tx| tx.input_notes().iter()).copied().collect();

        let created_notes: Vec<_> =
            txs.iter().flat_map(|tx| tx.output_notes().iter()).cloned().collect();

        if created_notes.len() > MAX_NOTES_PER_BATCH {
            return Err(BuildBatchError::TooManyNotesCreated(created_notes.len(), txs));
        }

        // TODO: document under what circumstances SMT creating can fail
        let created_notes_smt = BatchNoteTree::with_contiguous_leaves(
            created_notes.iter().map(|note| (note.id(), note.metadata())),
        )
        .map_err(|e| BuildBatchError::NotesSmtError(e, txs))?;

        Ok(Self {
            id,
            updated_accounts,
            produced_nullifiers,
            created_notes_smt,
            created_notes,
        })
    }

    // PUBLIC ACCESSORS
    // --------------------------------------------------------------------------------------------

    /// Returns the batch ID.
    pub fn id(&self) -> BatchId {
        self.id
    }

    /// Returns an iterator over (account_id, init_state_hash) tuples for accounts that were
    /// modified in this transaction batch.
    pub fn account_initial_states(&self) -> impl Iterator<Item = (AccountId, Digest)> + '_ {
        self.updated_accounts
            .iter()
            .map(|update| (update.account_id(), update.init_state_hash()))
    }

    /// Returns an iterator over (account_id, details, new_state_hash) tuples for accounts that were
    /// modified in this transaction batch.
    pub fn updated_accounts(&self) -> impl Iterator<Item = BlockAccountUpdate> + '_ {
        self.updated_accounts.iter().map(|update| {
            BlockAccountUpdate::new(
                update.account_id(),
                update.final_state_hash(),
                update.details().clone(),
            )
        })
    }

    /// Returns an iterator over produced nullifiers for all consumed notes.
    pub fn produced_nullifiers(&self) -> impl Iterator<Item = Nullifier> + '_ {
        self.produced_nullifiers.iter().cloned()
    }

    /// Returns the root hash of the created notes SMT.
    pub fn created_notes_root(&self) -> Digest {
        self.created_notes_smt.root()
    }

    /// Returns created notes list.
    pub fn created_notes(&self) -> &Vec<OutputNote> {
        &self.created_notes
    }

    // HELPER FUNCTIONS
    // --------------------------------------------------------------------------------------------

    fn compute_id(txs: &[ProvenTransaction]) -> BatchId {
        let mut buf = Vec::with_capacity(32 * txs.len());
        for tx in txs {
            buf.extend_from_slice(&tx.id().as_bytes());
        }
        Blake3_256::hash(&buf)
    }
}
