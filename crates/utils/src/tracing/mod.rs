pub mod grpc;

use opentelemetry::{Key, Value}; // TODO: do we want to leak these through our trait?

/// ...
pub trait OpenTelemetrySpanExt {
    fn set_attribute(&self, key: impl Into<Key>, value: impl Into<Value>);
    fn set_error(&self, err: &dyn std::error::Error);
}

impl OpenTelemetrySpanExt for tracing::Span {
    /// ...
    fn set_attribute(&self, key: impl Into<Key>, value: impl Into<Value>) {
        tracing_opentelemetry::OpenTelemetrySpanExt::set_attribute(self, key, value);
    }
    /// ...
    fn set_error(&self, _err: &dyn std::error::Error) {
        todo!();
    }
}
