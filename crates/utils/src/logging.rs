use anyhow::Result;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::WithTonicConfig;
use tracing::subscriber::{self, Subscriber};
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{layer::SubscriberExt, EnvFilter, Layer, Registry};

/// Configures tracing and optionally enables an open-telemetry OTLP exporter.
///
/// The open-telemetry configuration is controlled via environment variables as defined in the
/// [specification](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/protocol/exporter.md#opentelemetry-protocol-exporter)
pub fn setup_tracing(enable_otel: bool) -> Result<()> {
    let otel_layer = enable_otel.then_some(open_telemetry_layer());
    let subscriber = Registry::default().with(stdout_layer()).with(otel_layer);
    tracing::subscriber::set_global_default(subscriber).map_err(Into::into)
}

pub fn setup_logging() -> Result<()> {
    subscriber::set_global_default(subscriber())?;

    Ok(())
}

fn open_telemetry_layer<S>() -> Box<dyn tracing_subscriber::Layer<S> + Send + Sync + 'static>
where
    S: Subscriber + Sync + Send,
    for<'a> S: tracing_subscriber::registry::LookupSpan<'a>,
{
    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_tls_config(tonic::transport::ClientTlsConfig::new().with_native_roots())
        .build()
        .unwrap();

    let tracer = opentelemetry_sdk::trace::TracerProvider::builder()
        .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
        .build();

    let tracer = tracer.tracer("tracing-otel-subscriber");
    OpenTelemetryLayer::new(tracer).boxed()
}

#[cfg(not(feature = "tracing-forest"))]
fn stdout_layer<S>() -> Box<dyn tracing_subscriber::Layer<S> + Send + Sync + 'static>
where
    S: Subscriber,
    for<'a> S: tracing_subscriber::registry::LookupSpan<'a>,
{
    use tracing_subscriber::fmt::format::FmtSpan;

    tracing_subscriber::fmt::layer()
        .pretty()
        .compact()
        .with_level(true)
        .with_file(true)
        .with_line_number(true)
        .with_target(true)
        .with_span_events(FmtSpan::NEW | FmtSpan::CLOSE)
        .with_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            // axum logs rejections from built-in extracts on the trace level, so we enable this
            // manually.
            "info,axum::rejection=trace".into()
        }))
        .boxed()
}

#[cfg(feature = "tracing-forest")]
fn stdout_layer<S>() -> Box<dyn tracing_subscriber::Layer<S> + Send + Sync + 'static>
where
    S: Subscriber,
    for<'a> S: tracing_subscriber::registry::LookupSpan<'a>,
{
    tracing_forest::ForestLayer::default()
        .with_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| {
            // axum logs rejections from built-in extracts on the trace level, so we enable this
            // manually.
            "info,axum::rejection=trace".into()
        }))
        .boxed()
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
            "info,axum::rejection=trace".into()
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
            "info,axum::rejection=trace".into()
        }),
    )
}
