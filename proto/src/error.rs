#[derive(Copy, Clone, Debug, PartialEq)]
pub enum ParseError {
    HexError(hex::FromHexError),
    TooMuchData { expected: usize, got: usize },
    InsufficientData { expected: usize, got: usize },
    MissingLeafKey,
    NotAValidFelt,
    InvalidProof,
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
            ParseError::MissingLeafKey => {
                write!(f, "Tiered sparse merkle tree proof missing key")
            },
            ParseError::NotAValidFelt => {
                write!(f, "Value is not in the range 0..MODULUS")
            },
            ParseError::InvalidProof => {
                write!(f, "Received TSMT proof is invalid")
            },
        }
    }
}

impl From<hex::FromHexError> for ParseError {
    fn from(value: hex::FromHexError) -> Self {
        ParseError::HexError(value)
    }
}
