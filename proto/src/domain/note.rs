use crate::note;

// FROM
// ================================================================================================

impl From<note::Note> for note::NoteSyncRecord {
    fn from(value: note::Note) -> Self {
        Self {
            note_index: value.note_index,
            note_hash: value.note_hash,
            sender: value.sender,
            tag: value.tag,
            merkle_path: value.merkle_path,
        }
    }
}
