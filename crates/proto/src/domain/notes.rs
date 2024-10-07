use std::collections::{BTreeMap, BTreeSet};

use miden_objects::{
    notes::{NoteExecutionHint, NoteId, NoteInclusionProof, NoteMetadata, NoteTag, NoteType},
    Digest, Felt,
};

use crate::{
    convert,
    domain::blocks::BlockInclusionProof,
    errors::{ConversionError, MissingFieldHelper},
    generated::note::{
        NoteAuthenticationInfo as NoteAuthenticationInfoProto,
        NoteInclusionInBlockProof as NoteInclusionInBlockProofPb, NoteMetadata as NoteMetadataPb,
    },
    try_convert,
};

impl TryFrom<NoteMetadataPb> for NoteMetadata {
    type Error = ConversionError;

    fn try_from(value: NoteMetadataPb) -> Result<Self, Self::Error> {
        let sender = value
            .sender
            .ok_or_else(|| NoteMetadataPb::missing_field(stringify!(sender)))?
            .try_into()?;
        let note_type = NoteType::try_from(value.note_type as u64)?;
        let tag = NoteTag::from(value.tag);

        let execution_hint = NoteExecutionHint::try_from(value.execution_hint)?;

        let aux = Felt::try_from(value.aux).map_err(|_| ConversionError::NotAValidFelt)?;

        Ok(NoteMetadata::new(sender, note_type, tag, execution_hint, aux)?)
    }
}

impl From<NoteMetadata> for NoteMetadataPb {
    fn from(val: NoteMetadata) -> Self {
        let sender = Some(val.sender().into());
        let note_type = val.note_type() as u32;
        let tag = val.tag().into();
        let execution_hint: u64 = val.execution_hint().into();
        let aux = val.aux().into();

        crate::generated::note::NoteMetadata {
            sender,
            note_type,
            tag,
            execution_hint,
            aux,
        }
    }
}

impl From<(&NoteId, &NoteInclusionProof)> for NoteInclusionInBlockProofPb {
    fn from((note_id, proof): (&NoteId, &NoteInclusionProof)) -> Self {
        Self {
            note_id: Some(note_id.into()),
            block_num: proof.location().block_num(),
            note_index_in_block: proof.location().node_index_in_block().into(),
            merkle_path: Some(Into::into(proof.note_path())),
        }
    }
}

impl TryFrom<&NoteInclusionInBlockProofPb> for (NoteId, NoteInclusionProof) {
    type Error = ConversionError;

    fn try_from(
        proof: &NoteInclusionInBlockProofPb,
    ) -> Result<(NoteId, NoteInclusionProof), Self::Error> {
        Ok((
            Digest::try_from(
                proof
                    .note_id
                    .as_ref()
                    .ok_or(NoteInclusionInBlockProofPb::missing_field(stringify!(note_id)))?,
            )?
            .into(),
            NoteInclusionProof::new(
                proof.block_num,
                proof.note_index_in_block.try_into()?,
                proof
                    .merkle_path
                    .as_ref()
                    .ok_or(NoteInclusionInBlockProofPb::missing_field(stringify!(merkle_path)))?
                    .try_into()?,
            )?,
        ))
    }
}

#[derive(Clone, Default, Debug)]
pub struct NoteAuthenticationInfo {
    pub block_proofs: BTreeMap<u32, BlockInclusionProof>,
    pub note_proofs: BTreeMap<NoteId, NoteInclusionProof>,
}

impl NoteAuthenticationInfo {
    pub fn contains_note(&self, note: &NoteId) -> bool {
        self.note_proofs.contains_key(note)
    }

    pub fn note_ids(&self) -> BTreeSet<NoteId> {
        self.note_proofs.keys().copied().collect()
    }

    pub fn note_proofs(&self) -> BTreeMap<NoteId, (BlockInclusionProof, NoteInclusionProof)> {
        let mut proofs = BTreeMap::new();
        for (note, note_proof) in &self.note_proofs {
            let block_proof = self
                .block_proofs
                .get(&note_proof.location().block_num())
                // TODO: What do we want to do here?
                .expect("Block proof must be present for each note").clone();

            proofs.insert(*note, (block_proof, note_proof.clone()));
        }

        proofs
    }
}

impl From<NoteAuthenticationInfo> for NoteAuthenticationInfoProto {
    fn from(value: NoteAuthenticationInfo) -> Self {
        Self {
            note_proofs: convert(&value.note_proofs),
            block_proofs: convert(value.block_proofs.into_values()),
        }
    }
}

impl TryFrom<NoteAuthenticationInfoProto> for NoteAuthenticationInfo {
    type Error = ConversionError;

    fn try_from(value: NoteAuthenticationInfoProto) -> Result<Self, Self::Error> {
        let result = Self {
            block_proofs: try_convert(value.block_proofs)?,
            note_proofs: try_convert(&value.note_proofs)?,
        };

        Ok(result)
    }
}
