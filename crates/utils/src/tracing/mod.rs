pub mod grpc;

use opentelemetry::trace::Status as OtelStatus;
use opentelemetry::{Key, Value};
use sealed::sealed;

/// Utility functions based on [`tracing_opentelemetry::OpenTelemetrySpanExt`].
#[sealed]
pub trait OpenTelemetrySpanExt {
    fn set_attribute(&self, key: impl Into<Key>, value: impl Into<Value>);
    fn set_error(&self, err: &dyn std::error::Error);
}

#[sealed]
impl OpenTelemetrySpanExt for tracing::Span {
    /// Sets an attribute on `Span`.
    fn set_attribute(&self, key: impl Into<Key>, value: impl Into<Value>) {
        tracing_opentelemetry::OpenTelemetrySpanExt::set_attribute(self, key, value);
    }
    /// Sets a status on `Span` based on an error.
    fn set_error(&self, err: &dyn std::error::Error) {
        tracing_opentelemetry::OpenTelemetrySpanExt::set_status(
            self,
            OtelStatus::Error { description: format!("{err:?}").into() },
        );
    }
}
