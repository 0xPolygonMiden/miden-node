use miden_objects::{block::BlockNumber, crypto::merkle::MerklePath, BlockHeader};

use crate::{
    errors::{ConversionError, MissingFieldHelper},
    generated::block as proto,
};

// BLOCK HEADER
// ================================================================================================

impl From<&BlockHeader> for proto::BlockHeader {
    fn from(header: &BlockHeader) -> Self {
        Self {
            version: header.version(),
            prev_hash: Some(header.prev_hash().into()),
            block_num: header.block_num().as_u32(),
            chain_root: Some(header.chain_root().into()),
            account_root: Some(header.account_root().into()),
            nullifier_root: Some(header.nullifier_root().into()),
            note_root: Some(header.note_root().into()),
            tx_hash: Some(header.tx_hash().into()),
            kernel_root: Some(header.kernel_root().into()),
            proof_hash: Some(header.proof_hash().into()),
            timestamp: header.timestamp(),
        }
    }
}

impl From<BlockHeader> for proto::BlockHeader {
    fn from(header: BlockHeader) -> Self {
        (&header).into()
    }
}

impl TryFrom<&proto::BlockHeader> for BlockHeader {
    type Error = ConversionError;

    fn try_from(value: &proto::BlockHeader) -> Result<Self, Self::Error> {
        value.try_into()
    }
}

impl TryFrom<proto::BlockHeader> for BlockHeader {
    type Error = ConversionError;

    fn try_from(value: proto::BlockHeader) -> Result<Self, Self::Error> {
        Ok(BlockHeader::new(
            value.version,
            value
                .prev_hash
                .ok_or(proto::BlockHeader::missing_field(stringify!(prev_hash)))?
                .try_into()?,
            value.block_num.into(),
            value
                .chain_root
                .ok_or(proto::BlockHeader::missing_field(stringify!(chain_root)))?
                .try_into()?,
            value
                .account_root
                .ok_or(proto::BlockHeader::missing_field(stringify!(account_root)))?
                .try_into()?,
            value
                .nullifier_root
                .ok_or(proto::BlockHeader::missing_field(stringify!(nullifier_root)))?
                .try_into()?,
            value
                .note_root
                .ok_or(proto::BlockHeader::missing_field(stringify!(note_root)))?
                .try_into()?,
            value
                .tx_hash
                .ok_or(proto::BlockHeader::missing_field(stringify!(tx_hash)))?
                .try_into()?,
            value
                .kernel_root
                .ok_or(proto::BlockHeader::missing_field(stringify!(kernel_root)))?
                .try_into()?,
            value
                .proof_hash
                .ok_or(proto::BlockHeader::missing_field(stringify!(proof_hash)))?
                .try_into()?,
            value.timestamp,
        ))
    }
}

/// Data required to verify a block's inclusion proof.
#[derive(Clone, Debug)]
pub struct BlockInclusionProof {
    pub block_header: BlockHeader,
    pub mmr_path: MerklePath,
    pub chain_length: BlockNumber,
}

impl From<BlockInclusionProof> for proto::BlockInclusionProof {
    fn from(value: BlockInclusionProof) -> Self {
        Self {
            block_header: Some(value.block_header.into()),
            mmr_path: Some((&value.mmr_path).into()),
            chain_length: value.chain_length.as_u32(),
        }
    }
}

impl TryFrom<proto::BlockInclusionProof> for BlockInclusionProof {
    type Error = ConversionError;

    fn try_from(value: proto::BlockInclusionProof) -> Result<Self, ConversionError> {
        let result = Self {
            block_header: value
                .block_header
                .ok_or(proto::BlockInclusionProof::missing_field("block_header"))?
                .try_into()?,
            mmr_path: (&value
                .mmr_path
                .ok_or(proto::BlockInclusionProof::missing_field("mmr_path"))?)
                .try_into()?,
            chain_length: value.chain_length.into(),
        };

        Ok(result)
    }
}
