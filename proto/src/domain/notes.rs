use miden_objects::notes::NoteEnvelope;

use crate::generated::note;

// Note
// ================================================================================================

impl From<note::Note> for note::NoteSyncRecord {
    fn from(value: note::Note) -> Self {
        Self {
            note_index: value.note_index,
            note_id: value.note_id,
            sender: value.sender,
            tag: value.tag,
            merkle_path: value.merkle_path,
        }
    }
}

// NoteCreated
// ================================================================================================

impl From<(usize, usize, NoteEnvelope)> for note::NoteCreated {
    fn from((batch_idx, note_idx, note): (usize, usize, NoteEnvelope)) -> Self {
        Self {
            batch_index: batch_idx as u32,
            note_index: note_idx as u32,
            note_id: Some(note.note_id().into()),
            sender: Some(note.metadata().sender().into()),
            tag: note.metadata().tag().into(),
            details: None,
        }
    }
}
