use crate::{domain::account_input_record::AccountInputRecord, errors::ParseError, responses};

// FROM
// ================================================================================================

impl TryFrom<responses::AccountBlockInputRecord> for AccountInputRecord {
    type Error = ParseError;

    fn try_from(
        account_input_record: responses::AccountBlockInputRecord
    ) -> Result<Self, Self::Error> {
        Ok(Self {
            account_id: account_input_record
                .account_id
                .ok_or(ParseError::ProtobufMissingData)?
                .try_into()?,
            account_hash: account_input_record
                .account_hash
                .ok_or(ParseError::ProtobufMissingData)?
                .try_into()?,
            proof: account_input_record.proof.ok_or(ParseError::ProtobufMissingData)?.try_into()?,
        })
    }
}
