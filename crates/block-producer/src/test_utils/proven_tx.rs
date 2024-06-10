use std::ops::Range;

use miden_air::HashFunction;
use miden_objects::{
    accounts::AccountId,
    notes::{NoteHeader, NoteId, NoteMetadata, NoteType, Nullifier},
    transaction::{
        OutputNote, ProvenTransaction, ProvenTransactionBuilder, ToInputNoteCommitments,
    },
    vm::ExecutionProof,
    Digest, Felt, Hasher, ONE,
};
use winterfell::StarkProof;

use super::MockPrivateAccount;

pub struct MockProvenTxBuilder {
    account_id: AccountId,
    initial_account_hash: Digest,
    final_account_hash: Digest,
    notes_created: Option<Vec<OutputNote>>,
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

    pub fn nullifiers(mut self, nullifiers: Vec<Nullifier>) -> Self {
        self.nullifiers = Some(nullifiers);

        self
    }

    pub fn notes_created(mut self, notes: Vec<OutputNote>) -> Self {
        self.notes_created = Some(notes);

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
                let note_metadata =
                    NoteMetadata::new(self.account_id, NoteType::OffChain, 0.into(), ONE).unwrap();

                OutputNote::Header(NoteHeader::new(note_id.into(), note_metadata))
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
        .add_input_notes(
            self.nullifiers
                .unwrap_or_default()
                .iter()
                .copied()
                .map(NullifierToInputNoteCommitmentsWrapper),
        )
        .add_output_notes(self.notes_created.unwrap_or_default())
        .build()
        .unwrap()
    }
}

// TODO: This is a dirty workaround for the faster migration to the latest `miden-objects`.
//       We need to find a better way to do this.
struct NullifierToInputNoteCommitmentsWrapper(Nullifier);

impl ToInputNoteCommitments for NullifierToInputNoteCommitmentsWrapper {
    fn nullifier(&self) -> Nullifier {
        self.0
    }

    fn note_id(&self) -> Option<NoteId> {
        None
    }
}
