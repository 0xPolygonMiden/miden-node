use anyhow::Result;
use tracing::subscriber::{self, Subscriber};
use tracing_subscriber::EnvFilter;

pub fn setup_logging() -> Result<()> {
    subscriber::set_global_default(subscriber())?;

    Ok(())
}

#[cfg(not(feature = "tracing-forest"))]
pub fn subscriber() -> impl Subscriber + core::fmt::Debug {
    use tracing_subscriber::fmt::format::FmtSpan;

    tracing_subscriber::fmt()
        .pretty()
        .compact()
        .with_level(true)
        .with_file(true)
        .with_line_number(true)
        .with_target(true)
        .with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            // axum logs rejections from built-in extracts on the trace level, so we enable this
            // manually.
            format!("{}=info,axum::rejection=trace", env!("CARGO_CRATE_NAME")).into()
        }))
        .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
        .finish()
}

#[cfg(feature = "tracing-forest")]
pub fn subscriber() -> impl Subscriber + core::fmt::Debug {
    pub use tracing_forest::ForestLayer;
    pub use tracing_subscriber::{layer::SubscriberExt, Registry};

    Registry::default().with(ForestLayer::default()).with(
        EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            // axum logs rejections from built-in extracts on the trace level, so we enable this
            // manually.
            format!("{}=info,axum::rejection=trace", env!("CARGO_CRATE_NAME")).into()
        }),
    )
}
