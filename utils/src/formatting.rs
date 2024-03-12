use std::fmt::Display;

use itertools::Itertools;
use miden_objects::{
    crypto::{
        hash::{blake::Blake3Digest, Digest},
        utils::bytes_to_hex_string,
    },
    notes::{NoteEnvelope, Nullifier},
    transaction::{InputNotes, OutputNotes},
};

pub fn format_account_id(id: u64) -> String {
    format!("0x{id:x}")
}

pub fn format_opt<T: Display>(opt: Option<&T>) -> String {
    opt.map(ToString::to_string).unwrap_or("None".to_owned())
}

pub fn format_input_notes(notes: &InputNotes<Nullifier>) -> String {
    format_array(notes.iter().map(Nullifier::to_hex))
}

pub fn format_output_notes(notes: &OutputNotes<NoteEnvelope>) -> String {
    format_array(notes.iter().map(|envelope| {
        let metadata = envelope.metadata();
        format!(
            "{{ note_id: {}, note_metadata: {{sender: {}, tag: {} }}}}",
            envelope.note_id().to_hex(),
            metadata.sender(),
            metadata.tag(),
        )
    }))
}

pub fn format_map<'a, K: Display + 'a, V: Display + 'a>(
    map: impl IntoIterator<Item = (&'a K, &'a V)>
) -> String {
    let map_str = map.into_iter().map(|(key, val)| format!("{key}: {val}")).join(", ");
    if map_str.is_empty() {
        "None".to_owned()
    } else {
        format!("{{ {} }}", map_str)
    }
}

pub fn format_array(list: impl IntoIterator<Item = impl Display>) -> String {
    let comma_separated = list.into_iter().join(", ");
    if comma_separated.is_empty() {
        "None".to_owned()
    } else {
        format!("[{}]", comma_separated)
    }
}

pub fn format_blake3_digest(digest: Blake3Digest<32>) -> String {
    bytes_to_hex_string(digest.as_bytes())
}
