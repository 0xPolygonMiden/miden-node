use thiserror::Error;

use super::CREATED_NOTES_SMT_DEPTH;

const MAX_NUM_CREATED_NOTES_PER_BATCH: usize = 2usize.pow(CREATED_NOTES_SMT_DEPTH as u32);

#[derive(Error, Debug, PartialEq)]
pub enum BuildBatchError {
    #[error("dummy")]
    Dummy,
    #[error(
        "Too many notes in the batch. Got: {0}, max: {}",
        MAX_NUM_CREATED_NOTES_PER_BATCH
    )]
    TooManyNotes(usize),
}
