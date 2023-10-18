mod local;

use async_trait::async_trait;

/// Creates a client/server pair that communicate locally using tokio channels
pub use local::create_local_client_server_pair;

/// Errors related to the RPC mechanism itself
/// TODO: Make errors more descriptive
#[derive(Debug)]
pub enum RpcError {
    SendError,
    RecvError,
}

#[async_trait]
pub trait Rpc<Request, Response>: Send + Sync + 'static {
    async fn handle_request(
        &self,
        x: Request,
    ) -> Response;
}

#[async_trait]
pub trait RpcServer {
    async fn serve(mut self) -> Result<(), RpcError>;
}

#[async_trait]
pub trait RpcClient<Request, Response>: Send + Sync + 'static {
    async fn call(
        &mut self,
        x: Request,
    ) -> Result<Response, RpcError>;
}
