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

impl From<(u64, NoteEnvelope)> for note::NoteCreated {
    fn from((note_idx, note): (u64, NoteEnvelope)) -> Self {
        Self {
            note_id: Some(note.note_id().into()),
            sender: Some(note.metadata().sender().into()),
            tag: note.metadata().tag().into(),
            note_index: note_idx as u32,
        }
    }
}
