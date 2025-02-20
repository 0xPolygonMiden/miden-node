use std::collections::BTreeMap;

use miden_objects::{
    block::{AccountWitness, BlockHeader, BlockInputs, BlockNumber, NullifierWitness},
    crypto::merkle::MerklePath,
    note::{NoteId, NoteInclusionProof},
    transaction::ChainMmr,
    utils::{Deserializable, Serializable},
};

use crate::{
    errors::{ConversionError, MissingFieldHelper},
    generated::{
        block as proto, note::NoteInclusionInBlockProof, responses::GetBlockInputsResponse,
    },
    AccountWitnessRecord, NullifierWitnessRecord,
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

// BLOCK INPUTS
// ================================================================================================

impl From<BlockInputs> for GetBlockInputsResponse {
    fn from(inputs: BlockInputs) -> Self {
        let (
            prev_block_header,
            chain_mmr,
            account_witnesses,
            nullifier_witnesses,
            unauthenticated_note_proofs,
        ) = inputs.into_parts();

        GetBlockInputsResponse {
            latest_block_header: Some(prev_block_header.into()),
            account_witnesses: account_witnesses
                .into_iter()
                .map(|(id, witness)| {
                    let (initial_state_commitment, proof) = witness.into_parts();
                    AccountWitnessRecord {
                        account_id: id,
                        initial_state_commitment,
                        proof,
                    }
                    .into()
                })
                .collect(),
            nullifier_witnesses: nullifier_witnesses
                .into_iter()
                .map(|(nullifier, witness)| {
                    let proof = witness.into_proof();
                    NullifierWitnessRecord { nullifier, proof }.into()
                })
                .collect(),
            chain_mmr: chain_mmr.to_bytes(),
            unauthenticated_note_proofs: unauthenticated_note_proofs
                .iter()
                .map(NoteInclusionInBlockProof::from)
                .collect(),
        }
    }
}

impl TryFrom<GetBlockInputsResponse> for BlockInputs {
    type Error = ConversionError;

    fn try_from(response: GetBlockInputsResponse) -> Result<Self, Self::Error> {
        let latest_block_header: BlockHeader = response
            .latest_block_header
            .ok_or(proto::BlockHeader::missing_field("block_header"))?
            .try_into()?;

        let account_witnesses = response
            .account_witnesses
            .into_iter()
            .map(|entry| {
                let witness_record: AccountWitnessRecord = entry.try_into()?;
                Ok((
                    witness_record.account_id,
                    AccountWitness::new(
                        witness_record.initial_state_commitment,
                        witness_record.proof,
                    ),
                ))
            })
            .collect::<Result<BTreeMap<_, _>, ConversionError>>()?;

        let nullifier_witnesses = response
            .nullifier_witnesses
            .into_iter()
            .map(|entry| {
                let witness: NullifierWitnessRecord = entry.try_into()?;
                Ok((witness.nullifier, NullifierWitness::new(witness.proof)))
            })
            .collect::<Result<BTreeMap<_, _>, ConversionError>>()?;

        let unauthenticated_note_proofs = response
            .unauthenticated_note_proofs
            .iter()
            .map(<(NoteId, NoteInclusionProof)>::try_from)
            .collect::<Result<_, ConversionError>>()?;

        let chain_mmr = ChainMmr::read_from_bytes(&response.chain_mmr)
            .map_err(|source| ConversionError::deserialization_error("ChainMmr", source))?;

        Ok(BlockInputs::new(
            latest_block_header,
            chain_mmr,
            account_witnesses,
            nullifier_witnesses,
            unauthenticated_note_proofs,
        ))
    }
}
