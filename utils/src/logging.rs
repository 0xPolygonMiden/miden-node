use anyhow::Result;
use tracing::subscriber;
use tracing_subscriber::{self, fmt::format::FmtSpan, EnvFilter};

pub fn setup_logging() -> Result<()> {
    let subscriber = tracing_subscriber::fmt()
        .json()
        .compact()
        .with_level(true)
        .with_file(true)
        .with_line_number(true)
        .with_thread_ids(false)
        .with_thread_names(true)
        .with_target(true)
        .with_env_filter(EnvFilter::from_default_env())
        .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
        .finish();
    subscriber::set_global_default(subscriber)?;

    Ok(())
}
