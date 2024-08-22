use miden_objects::BlockHeader;

use crate::{
    errors::{ConversionError, MissingFieldHelper},
    generated::block,
};

// BLOCK HEADER
// ================================================================================================

impl From<&BlockHeader> for block::BlockHeader {
    fn from(header: &BlockHeader) -> Self {
        Self {
            version: header.version(),
            prev_hash: Some(header.prev_hash().into()),
            block_num: header.block_num(),
            chain_root: Some(header.chain_root().into()),
            account_root: Some(header.account_root().into()),
            nullifier_root: Some(header.nullifier_root().into()),
            note_root: Some(header.note_root().into()),
            tx_hash: Some(header.tx_hash().into()),
            proof_hash: Some(header.proof_hash().into()),
            timestamp: header.timestamp(),
        }
    }
}

impl From<BlockHeader> for block::BlockHeader {
    fn from(header: BlockHeader) -> Self {
        (&header).into()
    }
}

impl TryFrom<&block::BlockHeader> for BlockHeader {
    type Error = ConversionError;

    fn try_from(value: &block::BlockHeader) -> Result<Self, Self::Error> {
        value.clone().try_into()
    }
}

impl TryFrom<block::BlockHeader> for BlockHeader {
    type Error = ConversionError;

    fn try_from(value: block::BlockHeader) -> Result<Self, Self::Error> {
        Ok(BlockHeader::new(
            value.version,
            value
                .prev_hash
                .ok_or(block::BlockHeader::missing_field(stringify!(prev_hash)))?
                .try_into()?,
            value.block_num,
            value
                .chain_root
                .ok_or(block::BlockHeader::missing_field(stringify!(chain_root)))?
                .try_into()?,
            value
                .account_root
                .ok_or(block::BlockHeader::missing_field(stringify!(account_root)))?
                .try_into()?,
            value
                .nullifier_root
                .ok_or(block::BlockHeader::missing_field(stringify!(nullifier_root)))?
                .try_into()?,
            value
                .note_root
                .ok_or(block::BlockHeader::missing_field(stringify!(note_root)))?
                .try_into()?,
            value
                .tx_hash
                .ok_or(block::BlockHeader::missing_field(stringify!(tx_hash)))?
                .try_into()?,
            value
                .proof_hash
                .ok_or(block::BlockHeader::missing_field(stringify!(proof_hash)))?
                .try_into()?,
            value.timestamp,
        ))
    }
}
