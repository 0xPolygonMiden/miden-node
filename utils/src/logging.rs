use anyhow::Result;
use tracing::subscriber::{self, Subscriber};

pub fn setup_logging() -> Result<()> {
    subscriber::set_global_default(subscriber())?;

    Ok(())
}

#[cfg(not(feature = "tracing-forest"))]
pub fn subscriber() -> impl Subscriber + core::fmt::Debug {
    use tracing::level_filters::LevelFilter;
    use tracing_subscriber::{fmt::format::FmtSpan, EnvFilter};

    tracing_subscriber::fmt()
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
        .finish()
}

#[cfg(feature = "tracing-forest")]
pub fn subscriber() -> impl Subscriber + core::fmt::Debug {
    pub use tracing_forest::ForestLayer;
    pub use tracing_subscriber::{layer::SubscriberExt, Registry};

    Registry::default().with(ForestLayer::default())
}
