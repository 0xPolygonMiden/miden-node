use miden_objects::BlockHeader;

use crate::{block_header, error};

// INTO
// ================================================================================================

impl From<BlockHeader> for block_header::BlockHeader {
    fn from(header: BlockHeader) -> Self {
        Self {
            prev_hash: Some(header.prev_hash().into()),
            block_num: u64::from(header.block_num())
                .try_into()
                .expect("TODO: BlockHeader.block_num should be u64"),
            chain_root: Some(header.chain_root().into()),
            account_root: Some(header.account_root().into()),
            nullifier_root: Some(header.nullifier_root().into()),
            note_root: Some(header.note_root().into()),
            batch_root: Some(header.batch_root().into()),
            proof_hash: Some(header.proof_hash().into()),
            version: u64::from(header.version())
                .try_into()
                .expect("TODO: BlockHeader.version should be u64"),
            timestamp: header.timestamp().into(),
        }
    }
}

// FROM
// ================================================================================================

impl TryFrom<&block_header::BlockHeader> for BlockHeader {
    type Error = error::ParseError;

    fn try_from(value: &block_header::BlockHeader) -> Result<Self, Self::Error> {
        value.clone().try_into()
    }
}

impl TryFrom<block_header::BlockHeader> for BlockHeader {
    type Error = error::ParseError;

    fn try_from(value: block_header::BlockHeader) -> Result<Self, Self::Error> {
        Ok(BlockHeader::new(
            value.prev_hash.ok_or(error::ParseError::ProtobufMissingData)?.try_into()?,
            value.block_num,
            value.chain_root.ok_or(error::ParseError::ProtobufMissingData)?.try_into()?,
            value.account_root.ok_or(error::ParseError::ProtobufMissingData)?.try_into()?,
            value.nullifier_root.ok_or(error::ParseError::ProtobufMissingData)?.try_into()?,
            value.note_root.ok_or(error::ParseError::ProtobufMissingData)?.try_into()?,
            value.batch_root.ok_or(error::ParseError::ProtobufMissingData)?.try_into()?,
            value.proof_hash.ok_or(error::ParseError::ProtobufMissingData)?.try_into()?,
            value.version.into(),
            value.timestamp.into(),
        ))
    }
}
