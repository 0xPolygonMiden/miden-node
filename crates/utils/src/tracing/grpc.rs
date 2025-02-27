use super::OpenTelemetrySpanExt;

/// A [`trace_fn`](tonic::transport::server::Server) implementation for the block producer which
/// adds open-telemetry information to the span.
///
/// Creates an `info` span following the open-telemetry standard: `block-producer.rpc/{method}`.
/// Additionally also pulls in remote tracing context which allows the server trace to be connected
/// to the client's origin trace.
pub fn block_producer_trace_fn<T>(request: &http::Request<T>) -> tracing::Span {
    let span = if let Some("SubmitProvenTransaction") = request.uri().path().rsplit('/').next() {
        tracing::info_span!("block-producer.rpc/SubmitProvenTransaction")
    } else {
        tracing::info_span!("block-producer.rpc/Unknown")
    };

    span.set_parent(request);
    span.set_http_attributes(request);
    span
}

/// A [`trace_fn`](tonic::transport::server::Server) implementation for the store which adds
/// open-telemetry information to the span.
///
/// Creates an `info` span following the open-telemetry standard: `store.rpc/{method}`. Additionally
/// also pulls in remote tracing context which allows the server trace to be connected to the
/// client's origin trace.
pub fn store_trace_fn<T>(request: &http::Request<T>) -> tracing::Span {
    let span = match request.uri().path().rsplit('/').next() {
        Some("ApplyBlock") => tracing::info_span!("store.rpc/ApplyBlock"),
        Some("CheckNullifiers") => tracing::info_span!("store.rpc/CheckNullifiers"),
        Some("CheckNullifiersByPrefix") => tracing::info_span!("store.rpc/CheckNullifiersByPrefix"),
        Some("GetAccountDetails") => tracing::info_span!("store.rpc/GetAccountDetails"),
        Some("GetAccountProofs") => tracing::info_span!("store.rpc/GetAccountProofs"),
        Some("GetAccountStateDelta") => tracing::info_span!("store.rpc/GetAccountStateDelta"),
        Some("GetBlockByNumber") => tracing::info_span!("store.rpc/GetBlockByNumber"),
        Some("GetBlockHeaderByNumber") => tracing::info_span!("store.rpc/GetBlockHeaderByNumber"),
        Some("GetBlockInputs") => tracing::info_span!("store.rpc/GetBlockInputs"),
        Some("GetBatchInputs") => tracing::info_span!("store.rpc/GetBatchInputs"),
        Some("GetNotesById") => tracing::info_span!("store.rpc/GetNotesById"),
        Some("GetTransactionInputs") => tracing::info_span!("store.rpc/GetTransactionInputs"),
        Some("SyncNotes") => tracing::info_span!("store.rpc/SyncNotes"),
        Some("SyncState") => tracing::info_span!("store.rpc/SyncState"),
        _ => tracing::info_span!("store.rpc/Unknown"),
    };

    span.set_parent(request);
    span.set_http_attributes(request);
    span
}

/// Injects open-telemetry remote context into traces.
#[derive(Copy, Clone)]
pub struct OtelInterceptor;

impl tonic::service::Interceptor for OtelInterceptor {
    fn call(
        &mut self,
        mut request: tonic::Request<()>,
    ) -> Result<tonic::Request<()>, tonic::Status> {
        let ctx = tracing::Span::current().context();
        opentelemetry::global::get_text_map_propagator(|propagator| {
            propagator.inject_context(&ctx, &mut MetadataInjector(request.metadata_mut()));
        });

        Ok(request)
    }
}

struct MetadataInjector<'a>(&'a mut tonic::metadata::MetadataMap);
impl opentelemetry::propagation::Injector for MetadataInjector<'_> {
    /// Set a key and value in the `MetadataMap`.  Does nothing if the key or value are not valid
    /// inputs
    fn set(&mut self, key: &str, value: String) {
        if let Ok(key) = tonic::metadata::MetadataKey::from_bytes(key.as_bytes()) {
            if let Ok(val) = tonic::metadata::MetadataValue::try_from(&value) {
                self.0.insert(key, val);
            }
        }
    }
}
