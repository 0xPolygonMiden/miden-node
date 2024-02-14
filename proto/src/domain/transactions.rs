use std::{
    collections::BTreeMap,
    fmt::{Display, Formatter},
};

use miden_node_utils::formatting::{format_map, format_opt};
use miden_objects::Digest;

use crate::{
    domain::accounts::AccountState,
    errors::{MissingFieldHelper, ParseError},
    generated::responses::{GetTransactionInputsResponse, NullifierTransactionInputRecord},
};

// TRANSACTION INPUTS
// ================================================================================================

/// Information needed from the store to verify a transaction.
#[derive(Debug)]
pub struct TransactionInputs {
    /// The account state in the store corresponding to tx's account ID
    pub account_state: AccountState,

    /// Maps each consumed notes' nullifier to block number, where the note is consumed
    /// (`zero` means, that note isn't consumed yet)
    pub nullifiers: BTreeMap<Digest, u32>,
}

impl Display for TransactionInputs {
    fn fmt(
        &self,
        f: &mut Formatter<'_>,
    ) -> std::fmt::Result {
        f.write_fmt(format_args!(
            "{{ account_id: {}, account_hash: {}, nullifiers: {} }}",
            self.account_state.account_id,
            format_opt(self.account_state.account_hash.as_ref()),
            format_map(&self.nullifiers)
        ))
    }
}

impl From<TransactionInputs> for GetTransactionInputsResponse {
    fn from(tx_inputs: TransactionInputs) -> Self {
        Self {
            account_state: Some(tx_inputs.account_state.into()),
            nullifiers: tx_inputs
                .nullifiers
                .into_iter()
                .map(|(nullifier, block_num)| NullifierTransactionInputRecord {
                    nullifier: Some(nullifier.into()),
                    block_num,
                })
                .collect(),
        }
    }
}

impl TryFrom<GetTransactionInputsResponse> for TransactionInputs {
    type Error = ParseError;

    fn try_from(response: GetTransactionInputsResponse) -> Result<Self, Self::Error> {
        let account_state = response
            .account_state
            .ok_or(GetTransactionInputsResponse::missing_field(stringify!(account_state)))?
            .try_into()?;

        let mut nullifiers = BTreeMap::new();
        for nullifier_record in response.nullifiers {
            let nullifier = nullifier_record
                .nullifier
                .ok_or(NullifierTransactionInputRecord::missing_field(stringify!(nullifier)))?
                .try_into()?;

            nullifiers.insert(nullifier, nullifier_record.block_num);
        }

        Ok(Self {
            account_state,
            nullifiers,
        })
    }
}
