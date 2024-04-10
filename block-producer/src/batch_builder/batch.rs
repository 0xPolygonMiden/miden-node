use std::collections::BTreeMap;

use miden_node_proto::{domain::accounts::AccountUpdateDetails, generated::note::NoteCreated};
use miden_objects::{
    accounts::AccountId,
    batches::BatchNoteTree,
    crypto::hash::blake::{Blake3Digest, Blake3_256},
    notes::{NoteEnvelope, Nullifier},
    transaction::{AccountDetails, OutputNote},
    utils::serde::Serializable,
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
    updated_accounts: BTreeMap<AccountId, AccountStates>,
    produced_nullifiers: Vec<Nullifier>,
    /// The notes stored `created_notes_smt`
    created_notes_smt: BatchNoteTree,
    created_notes: Vec<NoteCreated>,
    created_note_envelopes: Vec<NoteEnvelope>,
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

        let updated_accounts = txs
            .iter()
            .map(|tx| {
                (
                    tx.account_id(),
                    AccountStates {
                        initial_state: tx.initial_account_hash(),
                        final_state: tx.final_account_hash(),
                        details: tx.account_details().cloned(),
                    },
                )
            })
            .collect();

        let produced_nullifiers =
            txs.iter().flat_map(|tx| tx.input_notes().iter()).cloned().collect();

        let (created_notes, created_notes_smt, created_note_envelopes) = {
            let created_note_envelopes: Vec<NoteEnvelope> = txs
                .iter()
                .flat_map(|tx| tx.output_notes().iter())
                .cloned()
                .map(NoteEnvelope::from)
                .collect();

            let created_notes: Vec<NoteCreated> = txs
                .iter()
                .flat_map(|tx| {
                    tx.output_notes().iter().map(|note| match note {
                        OutputNote::Public(note) => NoteCreated {
                            batch_index: 0,
                            note_index: 0,
                            note_id: Some(note.id().into()),
                            sender: Some(note.metadata().sender().into()),
                            tag: note.metadata().tag().into(),
                            details: Some(note.to_bytes()),
                        },
                        OutputNote::Private(note_envelope) => NoteCreated {
                            batch_index: 0,
                            note_index: 0,
                            note_id: Some(note_envelope.id().into()),
                            sender: Some(note_envelope.metadata().sender().into()),
                            tag: note_envelope.metadata().tag().into(),
                            details: None,
                        },
                    })
                })
                .collect();

            if created_notes.len() > MAX_NOTES_PER_BATCH {
                return Err(BuildBatchError::TooManyNotesCreated(created_notes.len(), txs));
            }

            // TODO: document under what circumstances SMT creating can fail
            (
                created_notes.clone(),
                BatchNoteTree::with_contiguous_leaves(
                    created_note_envelopes
                        .iter()
                        .map(|note_envelope| (note_envelope.id(), note_envelope.metadata())),
                )
                .map_err(|e| BuildBatchError::NotesSmtError(e, txs))?,
                created_note_envelopes,
            )
        };

        Ok(Self {
            id,
            updated_accounts,
            produced_nullifiers,
            created_notes_smt,
            created_notes,
            created_note_envelopes,
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
            .map(|(account_id, account_states)| (*account_id, account_states.initial_state))
    }

    /// Returns an iterator over (account_id, details, new_state_hash) tuples for accounts that were
    /// modified in this transaction batch.
    pub fn updated_accounts(&self) -> impl Iterator<Item = AccountUpdateDetails> + '_ {
        self.updated_accounts
            .iter()
            .map(|(&account_id, account_states)| AccountUpdateDetails {
                account_id,
                final_state_hash: account_states.final_state,
                details: account_states.details.clone(),
            })
    }

    /// Returns an iterator over produced nullifiers for all consumed notes.
    pub fn produced_nullifiers(&self) -> impl Iterator<Item = Nullifier> + '_ {
        self.produced_nullifiers.iter().cloned()
    }

    /// Returns an iterator over created notes.
    pub fn created_notes(&self) -> impl Iterator<Item = &NoteCreated> + '_ {
        self.created_notes.iter()
    }

    /// Returns the root hash of the created notes SMT.
    pub fn created_notes_root(&self) -> Digest {
        self.created_notes_smt.root()
    }

    #[allow(dead_code)]
    /// Returns an iterator over created note envelopes.
    pub fn created_notes_envelopes(&self) -> impl Iterator<Item = &NoteEnvelope> + '_ {
        self.created_note_envelopes.iter()
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

/// Stores the initial state (before the transaction) and final state (after the transaction) of an
/// account.
///
/// TODO: should this be moved into domain objects?
#[derive(Debug, Clone, PartialEq, Eq)]
struct AccountStates {
    initial_state: Digest,
    final_state: Digest,
    details: Option<AccountDetails>,
}
