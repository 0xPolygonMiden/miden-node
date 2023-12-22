use miden_node_proto::error::ParseError;
use thiserror::Error;

// TODO: consolidate errors in this file
#[derive(Debug, PartialEq, Error)]
pub enum TxInputsError {
    #[error("gRPC client failed with error: {0}")]
    GrpcClientError(String),
    #[error("malformed response from store: {0}")]
    MalformedResponse(String),
    #[error("failed to parse protobuf message: {0}")]
    ParseError(#[from] ParseError),
    #[error("dummy")]
    Dummy,
}

#[derive(Debug, PartialEq, Error)]
pub enum BlockInputsError {
    #[error("failed to parse protobuf message: {0}")]
    ParseError(#[from] ParseError),
    #[error("gRPC client failed with error: {0}")]
    GrpcClientError(String),
}

#[derive(Debug, PartialEq, Eq, Error)]
pub enum ApplyBlockError {
    #[error("gRPC client failed with error: {0}")]
    GrpcClientError(String),
}
