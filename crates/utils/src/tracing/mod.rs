pub mod grpc;

use core::time::Duration;
use miden_objects::block::BlockNumber;
use miden_objects::Digest;
use opentelemetry::trace::Status as OtelStatus;
use opentelemetry::{Key, Value};
use sealed::sealed;

/// Utility functions for converting types into [`opentelemetry::Value`].
pub trait ToValue {
    fn to_value(&self) -> Value;
}

impl ToValue for Duration {
    fn to_value(&self) -> Value {
        self.as_secs_f64().into()
    }
}

impl ToValue for Digest {
    fn to_value(&self) -> Value {
        self.to_hex().into()
    }
}

impl ToValue for f64 {
    fn to_value(&self) -> Value {
        (*self).into()
    }
}

impl ToValue for BlockNumber {
    fn to_value(&self) -> Value {
        i64::from(self.as_u32()).into()
    }
}

impl ToValue for u32 {
    fn to_value(&self) -> Value {
        i64::from(*self).into()
    }
}

impl ToValue for i64 {
    fn to_value(&self) -> Value {
        (*self).into()
    }
}

/// Utility functions based on [`tracing_opentelemetry::OpenTelemetrySpanExt`].
#[sealed]
pub trait OpenTelemetrySpanExt {
    fn set_attribute(&self, key: impl Into<Key>, value: impl ToValue);
    fn set_error(&self, err: &dyn std::error::Error);
}

#[sealed]
impl OpenTelemetrySpanExt for tracing::Span {
    /// Sets an attribute on `Span`.
    ///
    /// Implementations for `ToValue` should be added to this crate (miden-node-utils).
    fn set_attribute(&self, key: impl Into<Key>, value: impl ToValue) {
        tracing_opentelemetry::OpenTelemetrySpanExt::set_attribute(self, key, value.to_value());
    }

    /// Sets a status on `Span` based on an error.
    fn set_error(&self, err: &dyn std::error::Error) {
        tracing_opentelemetry::OpenTelemetrySpanExt::set_status(
            self,
            OtelStatus::Error { description: format!("{err:?}").into() },
        );
    }
}
