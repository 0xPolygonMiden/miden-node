use std::str::FromStr;

use anyhow::Result;
use opentelemetry::trace::TracerProvider as _;
use opentelemetry_otlp::WithTonicConfig;
use opentelemetry_sdk::propagation::TraceContextPropagator;
use tracing::subscriber::Subscriber;
use tracing_opentelemetry::OpenTelemetryLayer;
use tracing_subscriber::{
    layer::{Filter, SubscriberExt},
    Layer, Registry,
};

#[derive(Clone, Copy)]
pub enum OpenTelemetry {
    Enabled,
    Disabled,
}

impl OpenTelemetry {
    fn is_enabled(self) -> bool {
        matches!(self, OpenTelemetry::Enabled)
    }
}

/// Configures tracing and optionally enables an open-telemetry OTLP exporter.
///
/// The open-telemetry configuration is controlled via environment variables as defined in the
/// [specification](https://github.com/open-telemetry/opentelemetry-specification/blob/main/specification/protocol/exporter.md#opentelemetry-protocol-exporter)
pub fn setup_tracing(otel: OpenTelemetry) -> Result<()> {
    if otel.is_enabled() {
        opentelemetry::global::set_text_map_propagator(TraceContextPropagator::new());
    }

    // Note: open-telemetry requires a tokio-runtime, so this _must_ be lazily evaluated (aka not
    // `then_some`) to avoid crashing sync callers (with OpenTelemetry::Disabled set). Examples of
    // such callers are tests with logging enabled.
    let otel_layer = otel.is_enabled().then(open_telemetry_layer);

    let subscriber = Registry::default()
        .with(stdout_layer().with_filter(env_or_default_filter()))
        .with(otel_layer.with_filter(env_or_default_filter()));
    tracing::subscriber::set_global_default(subscriber).map_err(Into::into)
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

    let tracer = opentelemetry_sdk::trace::SdkTracerProvider::builder()
        .with_batch_exporter(exporter)
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
        .boxed()
}

#[cfg(feature = "tracing-forest")]
fn stdout_layer<S>() -> Box<dyn tracing_subscriber::Layer<S> + Send + Sync + 'static>
where
    S: Subscriber,
    for<'a> S: tracing_subscriber::registry::LookupSpan<'a>,
{
    tracing_forest::ForestLayer::default().boxed()
}

/// Creates a filter from the `RUST_LOG` env var with a default of `INFO` if unset.
///
/// # Panics
///
/// Panics if `RUST_LOG` fails to parse.
fn env_or_default_filter<S>() -> Box<dyn Filter<S> + Send + Sync + 'static> {
    use tracing::level_filters::LevelFilter;
    use tracing_subscriber::{
        filter::{FilterExt, Targets},
        EnvFilter,
    };

    // `tracing` does not allow differentiating between invalid and missing env var so we manually
    // do this instead. The alternative is to silently ignore parsing errors which I think is worse.
    match std::env::var(EnvFilter::DEFAULT_ENV) {
        Ok(rust_log) => FilterExt::boxed(
            EnvFilter::from_str(&rust_log)
                .expect("RUST_LOG should contain a valid filter configuration"),
        ),
        Err(std::env::VarError::NotUnicode(_)) => panic!("RUST_LOG contained non-unicode"),
        Err(std::env::VarError::NotPresent) => {
            // Default level is INFO, and additionally enable logs from axum extractor rejections.
            FilterExt::boxed(
                Targets::new()
                    .with_default(LevelFilter::INFO)
                    .with_target("axum::rejection", LevelFilter::TRACE),
            )
        },
    }
}
