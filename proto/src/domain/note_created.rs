use miden_objects::notes::NoteEnvelope;

use crate::note;

// INTO
// ================================================================================================

impl From<(u64, NoteEnvelope)> for note::NoteCreated {
    fn from((note_idx, note): (u64, NoteEnvelope)) -> Self {
        Self {
            note_hash: Some(note.note_id().into()),
            sender: note.metadata().sender().into(),
            tag: note.metadata().tag().into(),
            note_index: note_idx as u32,
        }
    }
}
