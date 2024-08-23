use std::ops::Range;

use miden_air::HashFunction;
use miden_objects::{
    accounts::AccountId,
    notes::{Note, NoteExecutionHint, NoteHeader, NoteMetadata, NoteType, Nullifier},
    transaction::{InputNote, OutputNote, ProvenTransaction, ProvenTransactionBuilder},
    vm::ExecutionProof,
    Digest, Felt, Hasher, ONE,
};
use winterfell::Proof;

use super::MockPrivateAccount;

pub struct MockProvenTxBuilder {
    account_id: AccountId,
    initial_account_hash: Digest,
    final_account_hash: Digest,
    output_notes: Option<Vec<OutputNote>>,
    input_notes: Option<Vec<InputNote>>,
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
            output_notes: None,
            input_notes: None,
            nullifiers: None,
        }
    }

    pub fn unauthenticated_notes(mut self, notes: Vec<Note>) -> Self {
        self.input_notes = Some(notes.into_iter().map(InputNote::unauthenticated).collect());

        self
    }

    pub fn nullifiers(mut self, nullifiers: Vec<Nullifier>) -> Self {
        self.nullifiers = Some(nullifiers);

        self
    }

    pub fn output_notes(mut self, notes: Vec<OutputNote>) -> Self {
        self.output_notes = Some(notes);

        self
    }

    pub fn nullifiers_range(self, range: Range<u64>) -> Self {
        let nullifiers = range
            .map(|index| {
                let nullifier = Digest::from([ONE, ONE, ONE, Felt::new(index)]);

                Nullifier::from(nullifier)
            })
            .collect();

        self.nullifiers(nullifiers)
    }

    pub fn private_notes_created_range(self, range: Range<u64>) -> Self {
        let notes = range
            .map(|note_index| {
                let note_id = Hasher::hash(&note_index.to_be_bytes());
                let note_metadata = NoteMetadata::new(
                    self.account_id,
                    NoteType::Private,
                    0.into(),
                    NoteExecutionHint::none(),
                    ONE,
                )
                .unwrap();

                OutputNote::Header(NoteHeader::new(note_id.into(), note_metadata))
            })
            .collect();

        self.output_notes(notes)
    }

    pub fn build(self) -> ProvenTransaction {
        ProvenTransactionBuilder::new(
            self.account_id,
            self.initial_account_hash,
            self.final_account_hash,
            Digest::default(),
            ExecutionProof::new(Proof::new_dummy(), HashFunction::Blake3_192),
        )
        .add_input_notes(self.input_notes.unwrap_or_default())
        .add_input_notes(self.nullifiers.unwrap_or_default())
        .add_output_notes(self.output_notes.unwrap_or_default())
        .build()
        .unwrap()
    }
}

pub fn mock_proven_tx(
    account_index: u8,
    unauthenticated_notes: Vec<Note>,
    output_notes: Vec<OutputNote>,
) -> ProvenTransaction {
    MockProvenTxBuilder::with_account_index(account_index.into())
        .unauthenticated_notes(unauthenticated_notes)
        .output_notes(output_notes)
        .build()
}
