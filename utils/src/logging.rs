use std::fmt::Display;

use anyhow::Result;
use itertools::Itertools;
use miden_crypto::hash::rpo::RpoDigest;
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

pub fn format_opt<T: Display>(opt: Option<&T>) -> String {
    opt.map(ToString::to_string).unwrap_or("None".to_string())
}

pub fn format_hashes(hashes: &[RpoDigest]) -> String {
    format!("[{}]", hashes.iter().map(RpoDigest::to_hex).join(", "))
}

pub fn format_input_notes(notes: &InputNotes<Nullifier>) -> String {
    format!(
        "{{ notes: [{}], commitment: {} }}",
        notes.iter().map(Nullifier::to_hex).join(", "),
        notes.commitment()
    )
}

pub fn format_output_notes(notes: &OutputNotes<NoteEnvelope>) -> String {
    format!(
        "{{ notes: [{}], commitment: {} }}",
        notes
            .iter()
            .map(|envelope| {
                let metadata = envelope.metadata();
                format!(
                    "{{ note_id: {}, note_metadata: {{sender: {}, tag: {} }}}}",
                    envelope.note_id().inner(),
                    metadata.sender(),
                    metadata.tag(),
                )
            })
            .join(", "),
        notes.commitment()
    )
}
