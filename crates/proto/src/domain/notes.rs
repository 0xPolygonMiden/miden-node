use miden_objects::{
    notes::{NoteExecutionHint, NoteId, NoteInclusionProof, NoteMetadata, NoteTag, NoteType},
    Digest, Felt,
};

use crate::{
    errors::{ConversionError, MissingFieldHelper},
    generated::note::{
        NoteInclusionInBlockProof as NoteInclusionInBlockProofPb, NoteMetadata as NoteMetadataPb,
    },
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
            note_index_in_block: proof.location().node_index_in_block(),
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
                proof.note_index_in_block,
                proof
                    .merkle_path
                    .as_ref()
                    .ok_or(NoteInclusionInBlockProofPb::missing_field(stringify!(merkle_path)))?
                    .try_into()?,
            )?,
        ))
    }
}
