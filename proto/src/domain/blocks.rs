use miden_objects::BlockHeader;

use crate::{
    errors::{ConversionError, MissingFieldHelper},
    generated::block_header,
};

// BLOCK HEADER
// ================================================================================================

impl From<BlockHeader> for block_header::BlockHeader {
    fn from(header: BlockHeader) -> Self {
        Self {
            prev_hash: Some(header.prev_hash().into()),
            block_num: header.block_num(),
            chain_root: Some(header.chain_root().into()),
            account_root: Some(header.account_root().into()),
            nullifier_root: Some(header.nullifier_root().into()),
            note_root: Some(header.note_root().into()),
            batch_root: Some(header.batch_root().into()),
            proof_hash: Some(header.proof_hash().into()),
            version: u64::from(header.version())
                .try_into()
                .expect("Failed to convert BlockHeader.version into u32"),
            timestamp: header.timestamp().into(),
        }
    }
}

impl TryFrom<&block_header::BlockHeader> for BlockHeader {
    type Error = ConversionError;

    fn try_from(value: &block_header::BlockHeader) -> Result<Self, Self::Error> {
        value.clone().try_into()
    }
}

impl TryFrom<block_header::BlockHeader> for BlockHeader {
    type Error = ConversionError;

    fn try_from(value: block_header::BlockHeader) -> Result<Self, Self::Error> {
        Ok(BlockHeader::new(
            value
                .prev_hash
                .ok_or(block_header::BlockHeader::missing_field(stringify!(prev_hash)))?
                .try_into()?,
            value.block_num,
            value
                .chain_root
                .ok_or(block_header::BlockHeader::missing_field(stringify!(chain_root)))?
                .try_into()?,
            value
                .account_root
                .ok_or(block_header::BlockHeader::missing_field(stringify!(account_root)))?
                .try_into()?,
            value
                .nullifier_root
                .ok_or(block_header::BlockHeader::missing_field(stringify!(nullifier_root)))?
                .try_into()?,
            value
                .note_root
                .ok_or(block_header::BlockHeader::missing_field(stringify!(note_root)))?
                .try_into()?,
            value
                .batch_root
                .ok_or(block_header::BlockHeader::missing_field(stringify!(batch_root)))?
                .try_into()?,
            value
                .proof_hash
                .ok_or(block_header::BlockHeader::missing_field(stringify!(proof_hash)))?
                .try_into()?,
            value.version.into(),
            value
                .timestamp
                .try_into()
                .expect("timestamp value is greater than or equal to the field modulus"),
        ))
    }
}
