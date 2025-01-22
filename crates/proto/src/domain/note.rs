use std::collections::{BTreeMap, BTreeSet};

use miden_objects::{
    note::{NoteExecutionHint, NoteId, NoteInclusionProof, NoteMetadata, NoteTag, NoteType},
    Digest, Felt,
};

use crate::{
    convert,
    domain::block::BlockInclusionProof,
    errors::{ConversionError, MissingFieldHelper},
    generated::note as proto,
    try_convert,
};

impl TryFrom<proto::NoteMetadata> for NoteMetadata {
    type Error = ConversionError;

    fn try_from(value: proto::NoteMetadata) -> Result<Self, Self::Error> {
        let sender = value
            .sender
            .ok_or_else(|| proto::NoteMetadata::missing_field(stringify!(sender)))?
            .try_into()?;
        let note_type = NoteType::try_from(u64::from(value.note_type))?;
        let tag = NoteTag::from(value.tag);

        let execution_hint = NoteExecutionHint::try_from(value.execution_hint)?;

        let aux = Felt::try_from(value.aux).map_err(|_| ConversionError::NotAValidFelt)?;

        Ok(NoteMetadata::new(sender, note_type, tag, execution_hint, aux)?)
    }
}

impl From<NoteMetadata> for proto::NoteMetadata {
    fn from(val: NoteMetadata) -> Self {
        let sender = Some(val.sender().into());
        let note_type = val.note_type() as u32;
        let tag = val.tag().into();
        let execution_hint: u64 = val.execution_hint().into();
        let aux = val.aux().into();

        proto::NoteMetadata {
            sender,
            note_type,
            tag,
            execution_hint,
            aux,
        }
    }
}

impl From<(&NoteId, &NoteInclusionProof)> for proto::NoteInclusionInBlockProof {
    fn from((note_id, proof): (&NoteId, &NoteInclusionProof)) -> Self {
        Self {
            note_id: Some(note_id.into()),
            block_num: proof.location().block_num().as_u32(),
            note_index_in_block: proof.location().node_index_in_block().into(),
            merkle_path: Some(Into::into(proof.note_path())),
        }
    }
}

impl TryFrom<&proto::NoteInclusionInBlockProof> for (NoteId, NoteInclusionProof) {
    type Error = ConversionError;

    fn try_from(
        proof: &proto::NoteInclusionInBlockProof,
    ) -> Result<(NoteId, NoteInclusionProof), Self::Error> {
        Ok((
            Digest::try_from(
                proof
                    .note_id
                    .as_ref()
                    .ok_or(proto::NoteInclusionInBlockProof::missing_field(stringify!(note_id)))?,
            )?
            .into(),
            NoteInclusionProof::new(
                proof.block_num.into(),
                proof.note_index_in_block.try_into()?,
                proof
                    .merkle_path
                    .as_ref()
                    .ok_or(proto::NoteInclusionInBlockProof::missing_field(stringify!(
                        merkle_path
                    )))?
                    .try_into()?,
            )?,
        ))
    }
}

#[derive(Clone, Default, Debug)]
pub struct NoteAuthenticationInfo {
    pub block_proofs: Vec<BlockInclusionProof>,
    pub note_proofs: BTreeMap<NoteId, NoteInclusionProof>,
}

impl NoteAuthenticationInfo {
    pub fn contains_note(&self, note: &NoteId) -> bool {
        self.note_proofs.contains_key(note)
    }

    pub fn note_ids(&self) -> BTreeSet<NoteId> {
        self.note_proofs.keys().copied().collect()
    }
}

impl From<NoteAuthenticationInfo> for proto::NoteAuthenticationInfo {
    fn from(value: NoteAuthenticationInfo) -> Self {
        Self {
            note_proofs: convert(&value.note_proofs),
            block_proofs: convert(value.block_proofs),
        }
    }
}

impl TryFrom<proto::NoteAuthenticationInfo> for NoteAuthenticationInfo {
    type Error = ConversionError;

    fn try_from(value: proto::NoteAuthenticationInfo) -> Result<Self, Self::Error> {
        let result = Self {
            block_proofs: try_convert(value.block_proofs)?,
            note_proofs: try_convert(&value.note_proofs)?,
        };

        Ok(result)
    }
}
