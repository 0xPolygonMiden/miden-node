use core::time::Duration;
use std::net::SocketAddr;

use miden_objects::{block::BlockNumber, Digest};
use opentelemetry::{trace::Status, Key, Value};

/// Utility functions for converting types into [`opentelemetry::Value`].
pub trait ToValue {
    fn to_value(&self) -> Value;
}

impl ToValue for Option<SocketAddr> {
    fn to_value(&self) -> Value {
        if let Some(socket_addr) = self {
            socket_addr.to_string().into()
        } else {
            "no_remote_addr".into()
        }
    }
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
///
/// This is a sealed trait. It and cannot be implemented outside of this module.
pub trait OpenTelemetrySpanExt: private::Sealed {
    fn set_parent<T>(&self, request: &http::Request<T>);
    fn set_attribute(&self, key: impl Into<Key>, value: impl ToValue);
    fn set_http_attributes<T>(&self, request: &http::Request<T>);
    fn set_error(&self, err: &dyn std::error::Error);
    fn context(&self) -> opentelemetry::Context;
}

impl<S> OpenTelemetrySpanExt for S
where
    S: tracing_opentelemetry::OpenTelemetrySpanExt,
{
    /// ...
    fn set_parent<T>(&self, request: &http::Request<T>) {
        // Pull the open-telemetry parent context using the HTTP extractor. We could make a more
        // generic gRPC extractor by utilising the gRPC metadata. However that
        //     (a) requires cloning headers,
        //     (b) we would have to write this ourselves, and
        //     (c) gRPC metadata is transferred using HTTP headers in any case.
        let otel_ctx = opentelemetry::global::get_text_map_propagator(|propagator| {
            propagator.extract(&MetadataExtractor(&tonic::metadata::MetadataMap::from_headers(
                request.headers().clone(),
            )))
        });
        tracing_opentelemetry::OpenTelemetrySpanExt::set_parent(self, otel_ctx);
    }

    /// ...
    fn context(&self) -> opentelemetry::Context {
        tracing_opentelemetry::OpenTelemetrySpanExt::context(self)
    }

    /// Sets an attribute on `Span`.
    ///
    /// Implementations for `ToValue` should be added to this crate (miden-node-utils).
    fn set_attribute(&self, key: impl Into<Key>, value: impl ToValue) {
        tracing_opentelemetry::OpenTelemetrySpanExt::set_attribute(self, key, value.to_value());
    }

    /// ...
    fn set_http_attributes<T>(&self, request: &http::Request<T>) {
        let remote_addr = request
            .extensions()
            .get::<tonic::transport::server::TcpConnectInfo>()
            .and_then(tonic::transport::server::TcpConnectInfo::remote_addr);
        OpenTelemetrySpanExt::set_attribute(self, "remote_addr", remote_addr);
    }

    /// Sets a status on `Span` based on an error.
    fn set_error(&self, err: &dyn std::error::Error) {
        // Coalesce all sources into one string.
        let mut description = format!("{err}");
        let current = err;
        while let Some(cause) = current.source() {
            description.push_str(format!("\nCaused by: {cause}").as_str());
        }
        tracing_opentelemetry::OpenTelemetrySpanExt::set_status(
            self,
            Status::Error { description: description.into() },
        );
    }
}

mod private {
    pub trait Sealed {}
    impl<S> Sealed for S where S: tracing_opentelemetry::OpenTelemetrySpanExt {}
}

/// ...
struct MetadataExtractor<'a>(pub(crate) &'a tonic::metadata::MetadataMap);
impl opentelemetry::propagation::Extractor for MetadataExtractor<'_> {
    /// Get a value for a key from the `MetadataMap`.  If the value can't be converted to &str,
    /// returns None
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).and_then(|metadata| metadata.to_str().ok())
    }

    /// Collect all the keys from the `MetadataMap`.
    fn keys(&self) -> Vec<&str> {
        self.0
            .keys()
            .map(|key| match key {
                tonic::metadata::KeyRef::Ascii(v) => v.as_str(),
                tonic::metadata::KeyRef::Binary(v) => v.as_str(),
            })
            .collect::<Vec<_>>()
    }
}
