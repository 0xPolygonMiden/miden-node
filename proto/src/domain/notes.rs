use miden_objects::notes::NoteEnvelope;

use crate::generated::note;

// NoteCreated
// ================================================================================================

impl From<&(usize, usize, NoteEnvelope)> for note::NoteCreated {
    fn from((batch_idx, note_idx, note): &(usize, usize, NoteEnvelope)) -> Self {
        Self {
            batch_index: *batch_idx as u32,
            note_index: *note_idx as u32,
            note_id: Some(note.note_id().into()),
            sender: Some(note.metadata().sender().into()),
            tag: note.metadata().tag().into(),
            // This is `None` for now as this conversion is used by the block-producer
            // when using `apply_block`. The block-producer has not yet been updated to support
            // on-chain notes. Will be set to the correct value when the block-producer is
            // up-to-date.
            details: None,
        }
    }
}
