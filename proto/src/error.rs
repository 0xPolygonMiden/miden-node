#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ParseError {
    HexError(hex::FromHexError),
    TooMuchData { expected: usize, got: usize },
    InsufficientData { expected: usize, got: usize },
}

impl std::error::Error for ParseError {}

impl std::fmt::Display for ParseError {
    fn fmt(
        &self,
        f: &mut std::fmt::Formatter<'_>,
    ) -> std::fmt::Result {
        match self {
            ParseError::HexError(e) => write!(f, "{}", e),
            ParseError::TooMuchData { expected, got } => {
                write!(f, "Too much data, expected {}, got {}", expected, got)
            },
            ParseError::InsufficientData { expected, got } => {
                write!(f, "Not enough data, expected {}, got {}", expected, got)
            },
        }
    }
}

impl From<hex::FromHexError> for ParseError {
    fn from(value: hex::FromHexError) -> Self {
        ParseError::HexError(value)
    }
}
