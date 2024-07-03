use std::{
    collections::{BTreeMap, BTreeSet},
    mem,
};

use miden_objects::{
    accounts::AccountId,
    batches::BatchNoteTree,
    block::BlockAccountUpdate,
    crypto::{
        hash::blake::{Blake3Digest, Blake3_256},
        merkle::MerklePath,
    },
    notes::{NoteId, Nullifier},
    transaction::{InputNoteCommitment, OutputNote, TransactionId, TxAccountUpdate},
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
    input_notes: Vec<InputNoteCommitment>,
    output_notes_smt: BatchNoteTree,
    output_notes: Vec<OutputNote>,
}

impl TransactionBatch {
    // CONSTRUCTORS
    // --------------------------------------------------------------------------------------------

    /// Returns a new [TransactionBatch] instantiated from the provided vector of proven
    /// transactions. If a map of unauthenticated notes found in the store is provided, it is used
    /// for transforming unauthenticated notes into authenticated notes.
    ///
    /// # Errors
    /// Returns an error if:
    /// - The number of output notes across all transactions exceeds 4096.
    /// - There are duplicated output notes or unauthenticated notes found across all transactions
    ///   in the batch.
    /// - Hashes for corresponding input notes and output notes don't match.
    ///
    /// TODO: enforce limit on the number of created nullifiers.
    #[instrument(target = "miden-block-producer", name = "new_batch", skip_all, err)]
    pub fn new(
        txs: Vec<ProvenTransaction>,
        found_unauthenticated_notes: Option<BTreeMap<NoteId, MerklePath>>,
    ) -> Result<Self, BuildBatchError> {
        let id = Self::compute_id(&txs);

        // Populate batch output notes and updated accounts.
        let mut updated_accounts = vec![];
        let mut output_notes = vec![];
        let mut output_note_index = BTreeMap::new();
        let mut unauthenticated_input_notes = BTreeSet::new();
        for tx in &txs {
            // TODO: we need to handle a possibility that a batch contains multiple transactions against
            //       the same account (e.g., transaction `x` takes account from state `A` to `B` and
            //       transaction `y` takes account from state `B` to `C`). These will need to be merged
            //       into a single "update" `A` to `C`.
            updated_accounts.push((tx.id(), tx.account_update().clone()));
            for note in tx.output_notes().iter() {
                if output_note_index.insert(note.id(), output_notes.len()).is_some() {
                    return Err(BuildBatchError::DuplicateOutputNote(note.id(), txs.clone()));
                }
                output_notes.push(Some(note.clone()));
            }
            // Check unauthenticated input notes for duplicates:
            for note in tx.get_unauthenticated_notes() {
                let id = note.id();
                if !unauthenticated_input_notes.insert(id) {
                    return Err(BuildBatchError::DuplicateUnauthenticatedNote(id, txs.clone()));
                }
            }
        }

        // Populate batch produced nullifiers and match output notes with corresponding
        // unauthenticated input notes in the same batch, which are removed from the unauthenticated
        // input notes set. We also don't add nullifiers for such output notes to the produced
        // nullifiers set.
        //
        // One thing to note:
        // This still allows transaction `A` to consume an unauthenticated note `x` and output note `y`
        // and for transaction `B` to consume an unauthenticated note `y` and output note `x`
        // (i.e., have a circular dependency between transactions), but this is not a problem.
        let mut input_notes = vec![];
        for input_note in txs.iter().flat_map(|tx| tx.input_notes().iter()) {
            // Header is presented only for unauthenticated notes.
            let input_note = match input_note.header() {
                Some(input_note_header) => {
                    let id = input_note_header.id();
                    if let Some(note_index) = output_note_index.remove(&id) {
                        if let Some(output_note) = mem::take(&mut output_notes[note_index]) {
                            let input_hash = input_note_header.hash();
                            let output_hash = output_note.hash();
                            if output_hash != input_hash {
                                return Err(BuildBatchError::NoteHashesMismatch {
                                    id,
                                    input_hash,
                                    output_hash,
                                    txs: txs.clone(),
                                });
                            }

                            // Don't add input notes if corresponding output notes consumed in the same batch.
                            continue;
                        }
                    }

                    match found_unauthenticated_notes {
                        Some(ref found_notes) => match found_notes.get(&input_note_header.id()) {
                            Some(_path) => input_note.nullifier().into(),
                            None => input_note.clone(),
                        },
                        None => input_note.clone(),
                    }
                },
                None => input_note.clone(),
            };
            input_notes.push(input_note)
        }

        let output_notes: Vec<_> = output_notes.into_iter().flatten().collect();

        if output_notes.len() > MAX_NOTES_PER_BATCH {
            return Err(BuildBatchError::TooManyNotesCreated(output_notes.len(), txs));
        }

        // Build the output notes SMT.
        let output_notes_smt = BatchNoteTree::with_contiguous_leaves(
            output_notes.iter().map(|note| (note.id(), note.metadata())),
        )
        .expect("Unreachable: fails only if the output note list contains duplicates");

        Ok(Self {
            id,
            updated_accounts,
            input_notes,
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

    /// Returns input notes list consumed by the transactions in this batch.
    pub fn input_notes(&self) -> &[InputNoteCommitment] {
        &self.input_notes
    }

    /// Returns an iterator over produced nullifiers for all consumed notes.
    pub fn produced_nullifiers(&self) -> impl Iterator<Item = Nullifier> + '_ {
        self.input_notes.iter().map(InputNoteCommitment::nullifier)
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
