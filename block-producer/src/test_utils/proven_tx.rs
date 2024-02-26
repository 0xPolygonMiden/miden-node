//! FibSmall taken from the `fib_small` example in `winterfell`

use std::sync::{Arc, Mutex};

use miden_air::{ExecutionProof, HashFunction};
use miden_objects::{
    accounts::AccountId,
    notes::{NoteEnvelope, NoteMetadata, Nullifier},
    transaction::{InputNotes, OutputNotes, ProvenTransaction},
    Digest, Felt, Hasher, ONE,
};
use once_cell::sync::Lazy;
use winterfell::StarkProof;

use super::MockPrivateAccount;

/// Keeps track how many accounts were created as a source of randomness
static NUM_ACCOUNTS_CREATED: Lazy<Arc<Mutex<u32>>> = Lazy::new(|| Arc::new(Mutex::new(0)));

/// Keeps track how many accounts were created as a source of randomness
static NUM_NOTES_CREATED: Lazy<Arc<Mutex<u64>>> = Lazy::new(|| Arc::new(Mutex::new(0)));

/// Keeps track how many input notes were created as a source of randomness
static NUM_INPUT_NOTES: Lazy<Arc<Mutex<u64>>> = Lazy::new(|| Arc::new(Mutex::new(0)));

pub struct MockProvenTxBuilder {
    account_id: AccountId,
    initial_account_hash: Digest,
    final_account_hash: Digest,
    notes_created: Option<Vec<NoteEnvelope>>,
    nullifiers: Option<Vec<Nullifier>>,
}

impl MockProvenTxBuilder {
    pub fn new() -> Self {
        let mock_account: MockPrivateAccount = {
            let mut locked_num_accounts_created = NUM_ACCOUNTS_CREATED.lock().unwrap();

            let account_index = *locked_num_accounts_created;

            *locked_num_accounts_created += 1;

            account_index.into()
        };

        Self::with_account(mock_account.id, mock_account.states[0], mock_account.states[1])
    }

    pub fn with_account(
        account_id: AccountId,
        initial_account_hash: Digest,
        final_account_hash: Digest,
    ) -> Self {
        Self {
            account_id,
            initial_account_hash,
            final_account_hash,
            notes_created: None,
            nullifiers: None,
        }
    }

    pub fn nullifiers(
        mut self,
        nullifiers: Vec<Nullifier>,
    ) -> Self {
        self.nullifiers = Some(nullifiers);
        self
    }

    pub fn notes_created(
        mut self,
        notes: Vec<NoteEnvelope>,
    ) -> Self {
        self.notes_created = Some(notes);
        self
    }

    pub fn num_notes_created(
        mut self,
        num_notes_created_in_tx: u64,
    ) -> Self {
        let mut locked_num_notes_created = NUM_NOTES_CREATED.lock().unwrap();

        let notes_created: Vec<_> = (*locked_num_notes_created
            ..(*locked_num_notes_created + num_notes_created_in_tx))
            .map(|note_index| {
                let note_hash = Hasher::hash(&note_index.to_be_bytes());

                NoteEnvelope::new(note_hash.into(), NoteMetadata::new(self.account_id, ONE))
            })
            .collect();

        // update state
        self.notes_created = Some(notes_created);
        *locked_num_notes_created += num_notes_created_in_tx;

        self
    }

    pub fn num_nullifiers(
        mut self,
        num_nullifiers_in_tx: u64,
    ) -> Self {
        let mut locked_num_input_notes = NUM_INPUT_NOTES.lock().unwrap();

        let nullifiers: Vec<Nullifier> = (0..num_nullifiers_in_tx)
            .map(|_| {
                *locked_num_input_notes += 1;

                let nullifier = Digest::from([
                    Felt::new(1),
                    Felt::new(1),
                    Felt::new(1),
                    Felt::new(*locked_num_input_notes),
                ]);

                Nullifier::from(nullifier)
            })
            .collect();

        self.nullifiers = Some(nullifiers);

        self
    }

    pub fn build(self) -> ProvenTransaction {
        ProvenTransaction::new(
            self.account_id,
            self.initial_account_hash,
            self.final_account_hash,
            InputNotes::new(self.nullifiers.unwrap_or_default()).unwrap(),
            OutputNotes::new(self.notes_created.unwrap_or_default()).unwrap(),
            None,
            Digest::default(),
            ExecutionProof::new(StarkProof::new_dummy(), HashFunction::Blake3_192),
        )
    }
}

impl Default for MockProvenTxBuilder {
    fn default() -> Self {
        Self::new()
    }
}
