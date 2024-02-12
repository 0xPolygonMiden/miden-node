use miden_objects::{crypto::merkle::MmrPeaks, BlockHeader};

use super::try_convert;
use crate::{
    errors::{MissingFieldHelper, ParseError},
    generated::{block_header, responses, responses::GetBlockInputsResponse},
    AccountInputRecord, NullifierWitness,
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
    type Error = ParseError;

    fn try_from(value: &block_header::BlockHeader) -> Result<Self, Self::Error> {
        value.clone().try_into()
    }
}

impl TryFrom<block_header::BlockHeader> for BlockHeader {
    type Error = ParseError;

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

// BLOCK INPUTS
// ================================================================================================

/// Information needed from the store to build a block
#[derive(Clone, Debug)]
pub struct BlockInputs {
    /// Previous block header
    pub block_header: BlockHeader,

    /// MMR peaks for the current chain state
    pub chain_peaks: MmrPeaks,

    /// The hashes of the requested accounts and their authentication paths
    pub account_states: Vec<AccountInputRecord>,

    /// The requested nullifiers and their authentication paths
    pub nullifiers: Vec<NullifierWitness>,
}

impl TryFrom<responses::GetBlockInputsResponse> for BlockInputs {
    type Error = ParseError;

    fn try_from(get_block_inputs: responses::GetBlockInputsResponse) -> Result<Self, Self::Error> {
        let block_header: BlockHeader = get_block_inputs
            .block_header
            .ok_or(GetBlockInputsResponse::missing_field(stringify!(block_header)))?
            .try_into()?;

        let chain_peaks = {
            // setting the number of leaves to the current block number gives us one leaf less than
            // what is currently in the chain MMR (i.e., chain MMR with block_num = 1 has 2 leave);
            // this is because GetBlockInputs returns the state of the chain MMR as of one block
            // ago so that block_header.chain_root matches the hash of MMR peaks.
            let num_leaves = block_header.block_num() as usize;

            MmrPeaks::new(
                num_leaves,
                get_block_inputs
                    .mmr_peaks
                    .into_iter()
                    .map(|peak| peak.try_into())
                    .collect::<Result<_, Self::Error>>()?,
            )
            .map_err(Self::Error::MmrPeaksError)?
        };

        Ok(Self {
            block_header,
            chain_peaks,
            account_states: try_convert(get_block_inputs.account_states)?,
            nullifiers: try_convert(get_block_inputs.nullifiers)?,
        })
    }
}
