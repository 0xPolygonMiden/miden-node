use crate::generated::note::{self, NoteCreated};

// NoteCreated
// ================================================================================================

impl From<&(usize, usize, NoteCreated)> for note::NoteCreated {
    fn from((batch_idx, note_idx, note): &(usize, usize, NoteCreated)) -> Self {
        Self {
            batch_index: *batch_idx as u32,
            note_index: *note_idx as u32,
            note_id: note.note_id.clone(),
            sender: note.sender.clone(),
            tag: note.tag,
            details: note.details.clone(),
        }
    }
}
