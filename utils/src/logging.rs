use std::fmt::Display;

use anyhow::Result;
use itertools::Itertools;
use miden_objects::{
    notes::{NoteEnvelope, Nullifier},
    transaction::{InputNotes, OutputNotes},
};
use tracing::{level_filters::LevelFilter, subscriber};
use tracing_subscriber::{self, fmt::format::FmtSpan, EnvFilter};

pub fn setup_logging() -> Result<()> {
    let subscriber = tracing_subscriber::fmt()
        .pretty()
        .compact()
        .with_level(true)
        .with_file(true)
        .with_line_number(true)
        .with_target(true)
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
        .finish();
    subscriber::set_global_default(subscriber)?;

    Ok(())
}

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
