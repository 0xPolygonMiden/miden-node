use std::ops::Range;

use miden_air::HashFunction;
use miden_objects::{
    accounts::AccountId,
    notes::{NoteEnvelope, NoteMetadata, NoteType, Nullifier},
    transaction::{ProvenTransaction, ProvenTransactionBuilder},
    vm::ExecutionProof,
    Digest, Felt, Hasher, ONE, ZERO,
};
use winterfell::StarkProof;

use super::MockPrivateAccount;

pub struct MockProvenTxBuilder {
    account_id: AccountId,
    initial_account_hash: Digest,
    final_account_hash: Digest,
    notes_created: Option<Vec<NoteEnvelope>>,
    nullifiers: Option<Vec<Nullifier>>,
}

impl MockProvenTxBuilder {
    pub fn with_account_index(account_index: u32) -> Self {
        let mock_account: MockPrivateAccount = account_index.into();

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

    pub fn nullifiers_range(
        self,
        range: Range<u64>,
    ) -> Self {
        let nullifiers = range
            .map(|index| {
                let nullifier = Digest::from([ONE, ONE, ONE, Felt::new(index)]);

                Nullifier::from(nullifier)
            })
            .collect();

        self.nullifiers(nullifiers)
    }

    pub fn notes_created_range(
        self,
        range: Range<u64>,
    ) -> Self {
        let notes = range
            .map(|note_index| {
                let note_hash = Hasher::hash(&note_index.to_be_bytes());

                NoteEnvelope::new(
                    note_hash.into(),
                    NoteMetadata::new(self.account_id, NoteType::OffChain, 0.into(), ONE).unwrap(),
                )
            })
            .collect();

        self.notes_created(notes)
    }

    pub fn build(self) -> ProvenTransaction {
        ProvenTransactionBuilder::new(
            self.account_id,
            self.initial_account_hash,
            self.final_account_hash,
            Digest::default(),
            ExecutionProof::new(StarkProof::new_dummy(), HashFunction::Blake3_192),
        )
        .add_input_notes(self.nullifiers.unwrap_or_default())
        .add_output_notes(self.notes_created.unwrap_or_default())
        .build()
        .unwrap()
    }
}
