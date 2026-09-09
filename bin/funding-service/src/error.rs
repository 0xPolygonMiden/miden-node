use miden_node_tracing::ErrorReport;
use miden_protocol::block::BlockNumber;

/// The reason a funding request failed.
#[derive(Debug, thiserror::Error)]
pub enum RequestFundsError {
    /// The service could not complete the request for a reason the client cannot act on.
    #[error("internal error")]
    Internal(#[source] anyhow::Error),

    /// A note must hold a non-zero amount.
    #[error("the requested amount must not be zero")]
    InvalidAmount,

    /// The request asked for more than the configured maximum.
    #[error("the requested amount {requested} exceeds the maximum of {maximum}")]
    AmountExceedsMaximum { requested: u64, maximum: u64 },

    /// The funding account cannot cover the request and the fee of one transaction.
    #[error(
        "the funding account holds {balance} base units, which does not cover the requested \
         {requested} plus a fee reserve of {reserve}"
    )]
    InsufficientFunds {
        requested: u64,
        balance: u64,
        reserve: u64,
    },

    /// The service cannot serve requests at the moment.
    #[error("the funding service is not ready: {0}")]
    NotReady(&'static str),

    /// The funding transaction did not commit before it expired.
    #[error("the funding transaction did not commit before block {expiration_block}")]
    TransactionExpired { expiration_block: BlockNumber },

    /// The node rejected the funding transaction.
    #[error("the node rejected the funding transaction")]
    TransactionRejected(#[source] anyhow::Error),

    /// Too many requests are queued.
    #[error("too many funding requests are queued")]
    Busy,
}

impl RequestFundsError {
    /// The byte code carried in the gRPC status details.
    fn code(&self) -> u8 {
        match self {
            Self::Internal(_) => 0,
            Self::InvalidAmount => 1,
            Self::AmountExceedsMaximum { .. } => 2,
            Self::InsufficientFunds { .. } => 3,
            Self::NotReady(_) => 4,
            Self::TransactionExpired { .. } => 5,
            Self::TransactionRejected(_) => 6,
            Self::Busy => 7,
        }
    }

    /// The gRPC status code for this error.
    fn status_code(&self) -> tonic::Code {
        match self {
            Self::Internal(_) => tonic::Code::Internal,
            Self::InvalidAmount | Self::AmountExceedsMaximum { .. } => tonic::Code::InvalidArgument,
            Self::InsufficientFunds { .. } => tonic::Code::FailedPrecondition,
            Self::NotReady(_) => tonic::Code::Unavailable,
            // `ABORTED` tells the client to retry the whole request. `DEADLINE_EXCEEDED` is not
            // used here because a tonic client also produces that code when its own deadline fires,
            // which would make the two cases indistinguishable.
            Self::TransactionExpired { .. } | Self::TransactionRejected(_) => tonic::Code::Aborted,
            Self::Busy => tonic::Code::ResourceExhausted,
        }
    }
}

impl From<RequestFundsError> for tonic::Status {
    fn from(err: RequestFundsError) -> Self {
        // An internal error may hold details about the service's own state, so the client only
        // receives a fixed message. The full report is logged by the worker.
        let message = match &err {
            RequestFundsError::Internal(_) => "internal error".to_owned(),
            other => other.as_report(),
        };

        Self::with_details(err.status_code(), message, vec![err.code()].into())
    }
}
