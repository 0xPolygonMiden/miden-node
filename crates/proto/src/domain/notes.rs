use miden_objects::{
    notes::{NoteMetadata, NoteTag, NoteType},
    Felt,
};

use crate::errors::{ConversionError, MissingFieldHelper};

impl TryFrom<crate::generated::note::NoteMetadata> for NoteMetadata {
    type Error = ConversionError;

    fn try_from(value: crate::generated::note::NoteMetadata) -> Result<Self, Self::Error> {
        let sender = value
            .sender
            .ok_or_else(|| crate::generated::note::NoteMetadata::missing_field("Sender"))?
            .try_into()?;
        let note_type = NoteType::try_from(value.note_type as u64)?;
        let tag = NoteTag::from(value.tag);
        let aux = Felt::new(value.aux);

        Ok(NoteMetadata::new(sender, note_type, tag, aux)?)
    }
}

impl TryInto<crate::generated::note::NoteMetadata> for NoteMetadata {
    type Error = ConversionError;

    fn try_into(self) -> Result<crate::generated::note::NoteMetadata, Self::Error> {
        let sender = Some(self.sender().into());
        let note_type = self.note_type() as i32;
        let tag = self.tag().into();
        let aux = self.aux().into();

        Ok(crate::generated::note::NoteMetadata { sender, note_type, tag, aux })
    }
}
