use std::collections::BTreeSet;

use miden_objects::{
    accounts::AccountId,
    batches::BatchNoteTree,
    block::BlockAccountUpdate,
    crypto::hash::blake::{Blake3Digest, Blake3_256},
    notes::{NoteId, Nullifier},
    transaction::{OutputNote, TransactionId, TxAccountUpdate},
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
    updated_accounts: Vec<(TransactionId, TxAccountUpdate)>,
    unauthenticated_input_notes: BTreeSet<NoteId>,
    produced_nullifiers: Vec<Nullifier>,
    output_notes_smt: BatchNoteTree,
    output_notes: Vec<OutputNote>,
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

        let mut updated_accounts = vec![];
        let mut produced_nullifiers = vec![];
        let mut unauthenticated_input_notes = BTreeSet::new();
        for tx in &txs {
            // TODO: we need to handle a possibility that a batch contains multiple transactions against
            //       the same account (e.g., transaction `x` takes account from state `A` to `B` and
            //       transaction `y` takes account from state `B` to `C`). These will need to be merged
            //       into a single "update" `A` to `C`.
            updated_accounts.push((tx.id(), tx.account_update().clone()));

            for note in tx.input_notes() {
                produced_nullifiers.push(note.nullifier());
                if let Some(header) = note.header() {
                    if !unauthenticated_input_notes.insert(header.id()) {
                        return Err(BuildBatchError::DuplicatedNoteId(header.id(), txs));
                    }
                }
            }
        }

        // Populate batch output notes, filtering out unauthenticated notes consumed in the same batch.
        // Consumed notes are also removed from the unauthenticated input notes set in order to avoid
        // consumption of notes with the same ID by one single input.
        //
        // One thing to note:
        // This still allows transaction `A` to consume an unauthenticated note `x` and output note `y`
        // and for transaction `B` to consume an unauthenticated note `y` and output note `x`
        // (i.e., have a circular dependency between transactions), but this is not a problem.
        let output_notes: Vec<_> = txs
            .iter()
            .flat_map(|tx| tx.output_notes().iter())
            .filter(|&note| !unauthenticated_input_notes.remove(&note.id()))
            .cloned()
            .collect();

        if output_notes.len() > MAX_NOTES_PER_BATCH {
            return Err(BuildBatchError::TooManyNotesCreated(output_notes.len(), txs));
        }

        // TODO: document under what circumstances SMT creating can fail
        let output_notes_smt = BatchNoteTree::with_contiguous_leaves(
            output_notes.iter().map(|note| (note.id(), note.metadata())),
        )
        .map_err(|e| BuildBatchError::NotesSmtError(e, txs))?;

        Ok(Self {
            id,
            updated_accounts,
            unauthenticated_input_notes,
            produced_nullifiers,
            output_notes_smt,
            output_notes,
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
            .map(|(_, update)| (update.account_id(), update.init_state_hash()))
    }

    /// Returns an iterator over (account_id, details, new_state_hash) tuples for accounts that were
    /// modified in this transaction batch.
    pub fn updated_accounts(&self) -> impl Iterator<Item = BlockAccountUpdate> + '_ {
        self.updated_accounts.iter().map(|(transaction_id, update)| {
            BlockAccountUpdate::new(
                update.account_id(),
                update.final_state_hash(),
                update.details().clone(),
                vec![*transaction_id],
            )
        })
    }

    /// Returns unauthenticated input notes set consumed by the transactions in this batch.
    pub fn unauthenticated_input_notes(&self) -> &BTreeSet<NoteId> {
        &self.unauthenticated_input_notes
    }

    /// Returns an iterator over produced nullifiers for all consumed notes.
    pub fn produced_nullifiers(&self) -> impl Iterator<Item = Nullifier> + '_ {
        self.produced_nullifiers.iter().cloned()
    }

    /// Returns the root hash of the output notes SMT.
    pub fn output_notes_root(&self) -> Digest {
        self.output_notes_smt.root()
    }

    /// Returns output notes list.
    pub fn output_notes(&self) -> &Vec<OutputNote> {
        &self.output_notes
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
