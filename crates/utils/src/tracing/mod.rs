pub mod grpc;

// Re-export useful traits for open-telemetry traces. This avoids requiring other crates from
// importing that family of crates directly.
pub use opentelemetry::trace::Status as OtelStatus;
pub use tracing_opentelemetry::OpenTelemetrySpanExt;
