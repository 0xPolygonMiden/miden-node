use std::collections::BTreeMap;

use miden_objects::{
    crypto::hash::rpo::RpoDigest,
    notes::{NoteId, NoteInclusionProof, NoteMetadata, NoteTag, NoteType},
    Felt,
};

use crate::{
    errors::{ConversionError, MissingFieldHelper},
    generated::{
        note::NoteMetadata as NoteMetadataPb,
        responses::BlockNoteInclusionProofs as BlockNoteInclusionProofsPb,
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

pub fn try_note_inclusion_proofs_from_proto(
    blocks: &[BlockNoteInclusionProofsPb],
) -> Result<BTreeMap<NoteId, NoteInclusionProof>, ConversionError> {
    blocks
        .iter()
        .flat_map(|block| block.notes.iter().map(move |proof| (block, proof)))
        .map(|(block, proof)| {
            Ok((
                RpoDigest::try_from(
                    proof
                        .note_id
                        .as_ref()
                        .ok_or(BlockNoteInclusionProofsPb::missing_field(stringify!(note_id)))?,
                )?
                .into(),
                miden_objects::notes::NoteInclusionProof::new(
                    block.block_num,
                    block
                        .sub_hash
                        .as_ref()
                        .ok_or(BlockNoteInclusionProofsPb::missing_field(stringify!(sub_hash)))?
                        .try_into()?,
                    block
                        .note_root
                        .as_ref()
                        .ok_or(BlockNoteInclusionProofsPb::missing_field(stringify!(note_root)))?
                        .try_into()?,
                    proof.note_index_in_block.into(),
                    proof
                        .merkle_path
                        .as_ref()
                        .ok_or(BlockNoteInclusionProofsPb::missing_field(stringify!(merkle_path)))?
                        .try_into()?,
                )?,
            ))
        })
        .collect()
}
