use miden_lib::transaction::TransactionKernel;
use miden_objects::{
    notes::Note,
    testing::notes::NoteBuilder,
    transaction::{InputNote, InputNoteCommitment, OutputNote},
};
use rand_chacha::{rand_core::SeedableRng, ChaCha20Rng};

use crate::test_utils::account::mock_account_id;

pub fn mock_note(num: u8) -> Note {
    let sender = mock_account_id(num);
    NoteBuilder::new(sender, ChaCha20Rng::from_seed([num; 32]))
        .build(&TransactionKernel::assembler().with_debug_mode(true))
        .unwrap()
}

pub fn mock_unauthenticated_note_commitment(num: u8) -> InputNoteCommitment {
    InputNote::unauthenticated(mock_note(num)).into()
}

pub fn mock_output_note(num: u8) -> OutputNote {
    OutputNote::Full(mock_note(num))
}