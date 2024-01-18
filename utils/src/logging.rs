use anyhow::Result;
use tracing::{level_filters::LevelFilter, subscriber};
use tracing_subscriber::{self, fmt::format::FmtSpan, EnvFilter};

pub fn setup_logging() -> Result<()> {
    let subscriber = tracing_subscriber::fmt()
        .pretty()
        .compact()
        .with_level(true)
        .with_file(true)
        .with_line_number(true)
        .with_target(false)
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .with_span_events(FmtSpan::ENTER | FmtSpan::EXIT)
        .finish();
    subscriber::set_global_default(subscriber)?;

    Ok(())
}

pub fn gen_request_id() -> u16 {
    // For now, it's just a random value. In future, we are going to get this value depending on context
    rand::random()
}
