//! New type wrappers describing a transaction's lifecycle in the context of the block producer.

use std::collections::{BTreeMap, BTreeSet};

use miden_air::ExecutionProof;
use miden_node_proto::domain::blocks::BlockInclusionProof;
use miden_objects::{
    accounts::AccountId,
    notes::{NoteHeader, NoteId, NoteInclusionProof, Nullifier},
    transaction::{OutputNotes, ProvenTransaction, TransactionId, TxAccountUpdate},
    Digest,
};

/// A transaction whose proof __has__ been validated.
#[derive(Debug, Clone)]
pub struct VerifiedTransaction {
    id: TransactionId,
    account_update: TxAccountUpdate,
    input_notes: InputNotes,
    output_notes: OutputNotes,
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
        VerifiedTransaction {
            id: tx.id(),
            account_update: tx.account_update().clone(),
            input_notes: tx.input_notes().clone().into(),
            output_notes: tx.output_notes().clone(),
            block_ref: tx.block_ref(),
            expiration_block_num: tx.expiration_block_num(),
            proof: tx.proof().clone(),
        }
    }

    pub fn nullifiers(&self) -> impl Iterator<Item = &Nullifier> {
        let unauthenticated = self.input_notes.unauthenticated.values().map(|note| &note.nullifier);
        let witnessed = self.input_notes.witnessed.values().map(|note| &note.nullifier);
        let proven = self.input_notes.proven.iter().map(|note| &note.0);

        unauthenticated.chain(witnessed).chain(proven)
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

    pub fn witness_note(
        &mut self,
        note_id: NoteId,
        block_proof: BlockInclusionProof,
        note_proof: NoteInclusionProof,
    ) -> Option<UnauthenticatedNote> {
        let note = self.input_notes.unauthenticated.remove(&note_id)?;

        self.input_notes.witnessed.insert(
            note_id,
            WitnessedNote {
                nullifier: note.nullifier,
                header: note.header,
                witness: (block_proof, note_proof),
            },
        );

        Some(note)
    }
}

#[derive(Debug, Clone)]
pub struct InputNotes {
    unauthenticated: BTreeMap<NoteId, UnauthenticatedNote>,
    witnessed: BTreeMap<NoteId, WitnessedNote>,
    proven: BTreeSet<ProvenNote>,
    commitment: Digest,
}

impl From<miden_objects::transaction::InputNotes<miden_objects::transaction::InputNoteCommitment>>
    for InputNotes
{
    fn from(
        value: miden_objects::transaction::InputNotes<
            miden_objects::transaction::InputNoteCommitment,
        >,
    ) -> Self {
        let commitment = value.commitment();
        let mut unauthenticated = BTreeMap::new();
        let mut proven = BTreeSet::new();

        for note in value {
            let nullifier = note.nullifier();

            match note.header().cloned() {
                Some(header) => {
                    unauthenticated.insert(header.id(), UnauthenticatedNote { nullifier, header });
                },
                None => {
                    proven.insert(ProvenNote(nullifier));
                },
            }
        }

        Self {
            unauthenticated,
            proven,
            witnessed: Default::default(),
            commitment,
        }
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd, Eq, Ord)]
pub struct ProvenNote(Nullifier);

#[derive(Debug, Clone, PartialEq)]
pub struct UnauthenticatedNote {
    nullifier: Nullifier,
    header: NoteHeader,
}

#[derive(Debug, Clone)]
pub struct WitnessedNote {
    nullifier: Nullifier,
    header: NoteHeader,
    witness: (BlockInclusionProof, NoteInclusionProof),
}

impl UnauthenticatedNote {
    pub fn witness_note(
        self,
        block_inclusion: BlockInclusionProof,
        note_inclusion: NoteInclusionProof,
    ) -> WitnessedNote {
        let Self { nullifier, header } = self;

        WitnessedNote {
            nullifier,
            header,
            witness: (block_inclusion, note_inclusion),
        }
    }
}
