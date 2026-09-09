use miden_protocol::account::AccountId;

use crate::decode::GrpcDecodeExt;
use crate::errors::ConversionError;
use crate::{decode, generated as proto};

// REQUEST FUNDS
// ================================================================================================

/// A request for native asset funds for one account.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestFunds {
    /// The account which the created note targets.
    pub account_id: AccountId,
    /// The amount of the native asset, in base units.
    pub amount: u64,
}

impl From<RequestFunds> for proto::funding_service::RequestFundsRequest {
    fn from(request: RequestFunds) -> Self {
        Self {
            account_id: Some(request.account_id.into()),
            amount: request.amount,
        }
    }
}

impl TryFrom<proto::funding_service::RequestFundsRequest> for RequestFunds {
    type Error = ConversionError;

    fn try_from(value: proto::funding_service::RequestFundsRequest) -> Result<Self, Self::Error> {
        let decoder = value.decoder();
        let proto::funding_service::RequestFundsRequest { account_id, amount } = value;

        let account_id = decode!(decoder, account_id)?;

        Ok(Self { account_id, amount })
    }
}
