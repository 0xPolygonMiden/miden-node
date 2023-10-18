use async_trait::async_trait;
use tokio::sync::mpsc::{error::SendError, unbounded_channel, UnboundedReceiver, UnboundedSender};

/// Creates a client/server pair that communicate locally using tokio channels
pub fn create_client_server_pair<Request, Response, RpcImpl>(
    rpc_impl: RpcImpl
) -> (RpcClient<Request, Response>, RpcServer<Request, Response, RpcImpl>)
where
    Request: Send + 'static,
    Response: Send + 'static,
    RpcImpl: Rpc<Request, Response>,
{
    let (client_send, server_recv) = unbounded_channel::<Request>();
    let (server_send, client_recv) = unbounded_channel::<Response>();

    let client = RpcClient {
        send: client_send,
        recv: client_recv,
    };

    let server = RpcServer {
        send: server_send,
        recv: server_recv,
        rpc_impl,
    };

    (client, server)
}

/// Errors related to the RPC mechanism itself
/// TODO: Make errors more descriptive
#[derive(Debug)]
pub enum RpcError {
    SendError,
    RecvError,
}

impl<T> From<SendError<T>> for RpcError {
    fn from(_send_error: SendError<T>) -> Self {
        Self::SendError
    }
}

#[async_trait]
pub trait Rpc<Request, Response>: Send + Sync + 'static {
    async fn handle_request(
        &self,
        x: Request,
    ) -> Response;
}

// RPC SERVER
// --------------------------------------------------------------------------------------

pub struct RpcServer<Request, Response, S>
where
    Request: Send + 'static,
    Response: Send + 'static,
    S: Rpc<Request, Response>,
{
    send: UnboundedSender<Response>,
    recv: UnboundedReceiver<Request>,
    rpc_impl: S,
}

impl<T, U, S> RpcServer<T, U, S>
where
    T: Send + 'static,
    U: Send + 'static,
    S: Rpc<T, U>,
{
    pub async fn serve(mut self) -> Result<(), RpcError> {
        loop {
            let request = self.recv.recv().await.ok_or(RpcError::RecvError)?;
            let response = self.rpc_impl.handle_request(request).await;
            self.send.send(response)?;
        }
    }
}

// RPC CLIENT
// --------------------------------------------------------------------------------------

pub struct RpcClient<Request, Response>
where
    Request: Send + 'static,
    Response: Send + 'static,
{
    send: UnboundedSender<Request>,
    recv: UnboundedReceiver<Response>,
}

impl<Request, Response> RpcClient<Request, Response>
where
    Request: Send + 'static,
    Response: Send + 'static,
{
    pub async fn call(
        &mut self,
        x: Request,
    ) -> Result<Response, RpcError> {
        self.send.send(x)?;
        self.recv.recv().await.ok_or(RpcError::RecvError)
    }
}
