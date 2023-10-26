#[derive(Copy, Clone, Debug, PartialEq)]
pub enum DbError {
    NoteMissingMerklePath,
    NoteMissingHash,
}

impl std::error::Error for DbError {}

impl std::fmt::Display for DbError {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            DbError::NoteMissingMerklePath => write!(f, "Note message is missing the merkle path"),
            DbError::NoteMissingHash => write!(f, "Note message is missing the note's hash"),
        }
    }
}
