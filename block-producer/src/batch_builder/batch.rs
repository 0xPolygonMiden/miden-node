use std::collections::BTreeMap;

use miden_crypto::hash::blake::{Blake3Digest, Blake3_256};
use miden_objects::{accounts::AccountId, notes::NoteEnvelope, Digest};
use miden_vm::crypto::SimpleSmt;
use tracing::instrument;

use super::errors::BuildBatchError;
use crate::{SharedProvenTx, CREATED_NOTES_SMT_DEPTH, MAX_NUM_CREATED_NOTES_PER_BATCH};

pub type BatchId = Blake3Digest<32>;

// TRANSACTION BATCH
// ================================================================================================

/// A batch of transactions that share a common proof. For any given account, at most 1 transaction
/// in the batch must be addressing that account (issue: #186).
///
/// Note: Until recursive proofs are available in the Miden VM, we don't include the common proof.
#[derive(Debug)]
pub struct TransactionBatch {
    id: BatchId,
    updated_accounts: BTreeMap<AccountId, AccountStates>,
    produced_nullifiers: Vec<Digest>,
    created_notes_smt: SimpleSmt<CREATED_NOTES_SMT_DEPTH>,
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
    #[instrument(target = "miden-block-producer", name = "new_batch", skip_all, err)]
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
                SimpleSmt::<CREATED_NOTES_SMT_DEPTH>::with_contiguous_leaves(
                    created_notes.into_iter().flat_map(|note_envelope| {
                        [note_envelope.note_id().into(), note_envelope.metadata().into()]
                    }),
                )?,
            )
        };

        let id = Self::calc_id(&txs);

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

    fn calc_id(txs: &[SharedProvenTx]) -> BatchId {
        let mut result = BatchId::default();

        for tx in txs {
            let tx_hash = Blake3_256::hash_elements(tx.id().inner().as_elements());
            result = Blake3_256::merge(&[result, tx_hash]);
        }

        result
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
