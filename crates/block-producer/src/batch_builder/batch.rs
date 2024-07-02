use std::collections::{BTreeMap, BTreeSet};

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
    output_notes: BTreeMap<NoteId, OutputNote>,
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

        // Populate batch output notes and updated accounts.
        let mut updated_accounts = vec![];
        let mut output_notes = BTreeMap::new();
        for tx in &txs {
            // TODO: we need to handle a possibility that a batch contains multiple transactions against
            //       the same account (e.g., transaction `x` takes account from state `A` to `B` and
            //       transaction `y` takes account from state `B` to `C`). These will need to be merged
            //       into a single "update" `A` to `C`.
            updated_accounts.push((tx.id(), tx.account_update().clone()));
            output_notes.extend(tx.output_notes().iter().map(|note| (note.id(), note.clone())));
        }

        // Populate batch unauthenticated input notes and produced nullifiers. Unauthenticated
        // input notes set doesn't contain output notes consumed in the same batch.
        // We also don't add nullifiers for such output notes to the produced nullifiers set.
        //
        // One thing to note:
        // This still allows transaction `A` to consume an unauthenticated note `x` and output note `y`
        // and for transaction `B` to consume an unauthenticated note `y` and output note `x`
        // (i.e., have a circular dependency between transactions), but this is not a problem.
        let mut unauthenticated_input_notes = BTreeSet::new();
        let mut produced_nullifiers = vec![];
        for input_note in txs.iter().flat_map(|tx| tx.input_notes().iter()) {
            if let Some(header) = input_note.header() {
                let note_id = header.id();
                if output_notes.remove(&note_id).is_some() {
                    // Don't produce nullifiers for output notes consumed in the same batch.
                    continue;
                } else {
                    unauthenticated_input_notes.insert(note_id);
                }
            }
            produced_nullifiers.push(input_note.nullifier());
        }

        if output_notes.len() > MAX_NOTES_PER_BATCH {
            return Err(BuildBatchError::TooManyNotesCreated(output_notes.len(), txs));
        }

        // Build the output notes SMT. Will fail if the output note list contains duplicates.
        let output_notes_smt = BatchNoteTree::with_contiguous_leaves(
            output_notes.iter().map(|(&id, note)| (id, note.metadata())),
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
    pub fn output_notes(&self) -> &BTreeMap<NoteId, OutputNote> {
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
