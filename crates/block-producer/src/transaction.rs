//! New type wrappers describing a transaction's lifecycle in the context of the block producer.

use std::collections::{BTreeMap, BTreeSet};

use miden_air::ExecutionProof;
use miden_node_proto::domain::blocks::BlockInclusionProof;
use miden_objects::{
    accounts::AccountId,
    notes::{Note, NoteHeader, NoteId, NoteInclusionProof, Nullifier},
    transaction::{
        InputNote, InputNoteCommitment, OutputNote, OutputNotes, ProvenTransaction, TransactionId,
        TxAccountUpdate,
    },
    Digest,
};

use crate::errors::InputNotesError;

/// A transaction whose proof __has__ been validated.
#[derive(Debug, Clone)]
pub struct VerifiedTransaction {
    id: TransactionId,
    account_update: TxAccountUpdate,
    input_notes: InputNotes,
    output_notes: BTreeMap<NoteId, OutputNote>,
    block_ref: Digest,
    expiration_block_num: u32,
    proof: ExecutionProof,
}

impl VerifiedTransaction {
    /// Creates a new [VerifiedTransaction] without verifying the proof.
    ///
    /// The caller is responsible for ensuring the validity of the proof prior to calling
    /// this method.
    pub fn new_unchecked(tx: ProvenTransaction) -> Self {
        let output_notes =
            tx.output_notes().iter().cloned().map(|note| (note.id(), note)).collect();
        VerifiedTransaction {
            id: tx.id(),
            account_update: tx.account_update().clone(),
            input_notes: tx.input_notes().clone().into(),
            output_notes,
            block_ref: tx.block_ref(),
            expiration_block_num: tx.expiration_block_num(),
            proof: tx.proof().clone(),
        }
    }

    pub fn nullifiers(&self) -> &BTreeSet<Nullifier> {
        &self.input_notes().nullifiers
    }

    pub fn unauthenticated_notes(&self) -> impl Iterator<Item = &NoteId> {
        self.input_notes.unauthenticated.keys()
    }

    pub fn account_update(&self) -> &TxAccountUpdate {
        &self.account_update
    }

    pub fn account_id(&self) -> AccountId {
        self.account_update.account_id()
    }

    pub fn id(&self) -> TransactionId {
        self.id
    }

    pub fn input_notes(&self) -> &InputNotes {
        &self.input_notes
    }

    pub fn output_notes(&self) -> &BTreeMap<NoteId, OutputNote> {
        &self.output_notes
    }

    /// Returns true if the witness was applied.
    ///
    /// Returns false if no such unauthenticated note was found.
    pub fn witness_note(
        &mut self,
        note_id: NoteId,
        block_proof: BlockInclusionProof,
        note_proof: NoteInclusionProof,
    ) -> bool {
        self.input_notes.witness_note(note_id, block_proof, note_proof)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct InputNotes {
    unauthenticated: BTreeMap<NoteId, UnauthenticatedNote>,
    witnessed: BTreeMap<NoteId, WitnessedNote>,
    proven: BTreeSet<ProvenNote>,
    /// Nullifiers of all the input notes in this set.
    nullifiers: BTreeSet<Nullifier>,
}

impl InputNotes {
    /// Merges `other` into `self`.
    ///
    /// # Errors
    ///
    /// Errors if the other set contains a duplicate nullifier.
    ///
    /// Note that this action is __not atomic__.
    pub fn merge(&mut self, other: Self) -> Result<(), BTreeSet<Nullifier>> {
        let duplicates = self
            .nullifiers
            .intersection(&other.nullifiers)
            .copied()
            .collect::<BTreeSet<_>>();
        if !duplicates.is_empty() {
            return Err(duplicates);
        }

        self.nullifiers.extend(other.nullifiers);
        self.unauthenticated.extend(other.unauthenticated);
        self.witnessed.extend(other.witnessed);
        self.proven.extend(other.proven);

        Ok(())
    }

    pub fn unauthenticated_notes(&self) -> impl Iterator<Item = &NoteId> {
        self.unauthenticated.keys()
    }

    pub fn witness_note(
        &mut self,
        note_id: NoteId,
        block_proof: BlockInclusionProof,
        note_proof: NoteInclusionProof,
    ) -> bool {
        let Some(note) = self.unauthenticated.remove(&note_id) else {
            return false;
        };

        self.witnessed.insert(note_id, note.witness_note(block_proof, note_proof));

        true
    }

    pub fn remove_unauthenticated(&mut self, id: &NoteId) -> Option<UnauthenticatedNote> {
        self.unauthenticated.remove(id)
    }

    pub fn len(&self) -> usize {
        self.unauthenticated.len() + self.witnessed.len() + self.proven.len()
    }

    pub fn into_input_note_commitments(self) -> impl Iterator<Item = InputNoteCommitment> {
        let unauthenticated = self.unauthenticated.into_values().map(|note| note.commitment);
        let witnessed = self.witnessed.into_values().map(|note| note.commitment);
        let proven = self.proven.into_iter().map(|note| note.0.into());

        unauthenticated.chain(witnessed).chain(proven)
    }
}

impl From<miden_objects::transaction::InputNotes<miden_objects::transaction::InputNoteCommitment>>
    for InputNotes
{
    fn from(
        value: miden_objects::transaction::InputNotes<
            miden_objects::transaction::InputNoteCommitment,
        >,
    ) -> Self {
        let mut unauthenticated = BTreeMap::new();
        let mut proven = BTreeSet::new();
        let mut nullifiers = BTreeSet::new();

        for note in value {
            let nullifier = note.nullifier();
            nullifiers.insert(nullifier);

            match note.header().cloned() {
                Some(header) => {
                    unauthenticated.insert(
                        header.id(),
                        UnauthenticatedNote { nullifier, header, commitment: note },
                    );
                },
                None => {
                    proven.insert(ProvenNote(nullifier));
                },
            }
        }

        Self {
            unauthenticated,
            proven,
            nullifiers,
            witnessed: Default::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub struct ProvenNote(Nullifier);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnauthenticatedNote {
    nullifier: Nullifier,
    header: NoteHeader,
    /// Kept purely to facilitate conversions since we cannot create this
    /// ourselves. This is completely redundant with what we have already.
    commitment: InputNoteCommitment,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WitnessedNote {
    nullifier: Nullifier,
    header: NoteHeader,
    witness: (BlockInclusionProof, NoteInclusionProof),
    /// Kept purely to facilitate conversions since we cannot create this
    /// ourselves. This is completely redundant with what we have already.
    commitment: InputNoteCommitment,
}

impl UnauthenticatedNote {
    pub fn witness_note(
        self,
        block_inclusion: BlockInclusionProof,
        note_inclusion: NoteInclusionProof,
    ) -> WitnessedNote {
        let Self { nullifier, header, commitment } = self;

        WitnessedNote {
            nullifier,
            header,
            // Drop the header.
            commitment: commitment.nullifier().into(),
            witness: (block_inclusion, note_inclusion),
        }
    }
}
