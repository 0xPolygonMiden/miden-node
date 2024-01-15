use std::collections::BTreeMap;

use miden_objects::{accounts::AccountId, notes::NoteEnvelope, Digest};
use miden_vm::crypto::SimpleSmt;

use super::errors::BuildBatchError;
use crate::{SharedProvenTx, CREATED_NOTES_SMT_DEPTH, MAX_NUM_CREATED_NOTES_PER_BATCH};

// TRANSACTION BATCH
// ================================================================================================

/// A batch of transactions that share a common proof. For any given account, at most 1 transaction
/// in the batch must be addressing that account.
///
/// Note: Until recursive proofs are available in the Miden VM, we don't include the common proof.
#[derive(Debug)]
pub struct TransactionBatch {
    updated_accounts: BTreeMap<AccountId, AccountStates>,
    produced_nullifiers: Vec<Digest>,
    created_notes_smt: SimpleSmt,
    /// The notes stored `created_notes_smt`
    created_notes: Vec<NoteEnvelope>,
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
            .flat_map(|tx| tx.input_notes().iter())
            .map(|nullifier| nullifier.inner())
            .collect();

        let (created_notes, created_notes_smt) = {
            let created_notes: Vec<NoteEnvelope> =
                txs.iter().flat_map(|tx| tx.output_notes().iter()).cloned().collect();

            if created_notes.len() > MAX_NUM_CREATED_NOTES_PER_BATCH {
                return Err(BuildBatchError::TooManyNotesCreated(created_notes.len()));
            }

            // TODO: document under what circumstances SMT creating can fail
            (
                created_notes.clone(),
                SimpleSmt::with_contiguous_leaves(
                    CREATED_NOTES_SMT_DEPTH,
                    created_notes.into_iter().flat_map(|note_envelope| {
                        [note_envelope.note_id().into(), note_envelope.metadata().into()]
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

    // PUBLIC ACCESSORS
    // --------------------------------------------------------------------------------------------

    /// Returns an iterator over (account_id, init_state_hash) tuples for accounts that were
    /// modified in this transaction batch.
    pub fn account_initial_states(&self) -> impl Iterator<Item = (AccountId, Digest)> + '_ {
        self.updated_accounts
            .iter()
            .map(|(account_id, account_states)| (*account_id, account_states.initial_state))
    }

    /// Returns an iterator over (account_id, new_state_hash) tuples for accounts that were
    /// modified in this transaction batch.
    pub fn updated_accounts(&self) -> impl Iterator<Item = (AccountId, Digest)> + '_ {
        self.updated_accounts
            .iter()
            .map(|(account_id, account_states)| (*account_id, account_states.final_state))
    }

    /// Returns the nullifier of all consumed notes.
    pub fn produced_nullifiers(&self) -> impl Iterator<Item = Digest> + '_ {
        self.produced_nullifiers.iter().cloned()
    }

    /// Returns the hash of created notes.
    pub fn created_notes(&self) -> impl Iterator<Item = &NoteEnvelope> + '_ {
        self.created_notes.iter()
    }

    /// Returns the root of the created notes SMT.
    pub fn created_notes_root(&self) -> Digest {
        self.created_notes_smt.root()
    }
}

/// Stores the initial state (before the transaction) and final state (after the transaction) of an
/// account.
///
/// TODO: should this be moved into domain objects?
#[derive(Debug)]
struct AccountStates {
    initial_state: Digest,
    final_state: Digest,
}
