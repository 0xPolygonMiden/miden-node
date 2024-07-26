use miden_objects::{
    crypto::{hash::rpo::RpoDigest, merkle::MerklePath},
    notes::{NoteId, NoteMetadata, NoteTag, NoteType},
    Felt,
};

use crate::{
    errors::{ConversionError, MissingFieldHelper},
    generated::note::{NoteInclusionProof as NoteInclusionProofPb, NoteMetadata as NoteMetadataPb},
};

impl TryFrom<NoteMetadataPb> for NoteMetadata {
    type Error = ConversionError;

    fn try_from(value: NoteMetadataPb) -> Result<Self, Self::Error> {
        let sender = value
            .sender
            .ok_or_else(|| crate::generated::note::NoteMetadata::missing_field("Sender"))?
            .try_into()?;
        let note_type = NoteType::try_from(value.note_type as u64)?;
        let tag = NoteTag::from(value.tag);
        let aux = Felt::try_from(value.aux).map_err(|_| ConversionError::NotAValidFelt)?;

        Ok(NoteMetadata::new(sender, note_type, tag, aux)?)
    }
}

impl From<NoteMetadata> for NoteMetadataPb {
    fn from(val: NoteMetadata) -> Self {
        let sender = Some(val.sender().into());
        let note_type = val.note_type() as u32;
        let tag = val.tag().into();
        let aux = val.aux().into();

        NoteMetadataPb { sender, note_type, tag, aux }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct NoteInclusionProof {
    pub note_id: NoteId,
    pub merkle_path: MerklePath,
}

impl NoteInclusionProof {
    pub fn into_parts(self) -> (NoteId, MerklePath) {
        (self.note_id, self.merkle_path)
    }
}

impl From<NoteInclusionProof> for NoteInclusionProofPb {
    fn from(value: NoteInclusionProof) -> Self {
        Self {
            note_id: Some(value.note_id.into()),
            merkle_path: Some(value.merkle_path.into()),
        }
    }
}

impl TryFrom<NoteInclusionProofPb> for NoteInclusionProof {
    type Error = ConversionError;

    fn try_from(proof: NoteInclusionProofPb) -> Result<Self, Self::Error> {
        Ok(Self {
            note_id: RpoDigest::try_from(
                proof.note_id.ok_or(NoteInclusionProofPb::missing_field(stringify!(note_id)))?,
            )?
            .into(),
            merkle_path: proof
                .merkle_path
                .ok_or(NoteInclusionProofPb::missing_field(stringify!(merkle_path)))?
                .try_into()?,
        })
    }
}
