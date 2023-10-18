use async_trait::async_trait;
use tokio::sync::mpsc::{error::SendError, unbounded_channel, UnboundedReceiver, UnboundedSender};

use super::{Rpc, RpcClient, RpcError, RpcServer};

pub fn create_local_client_server_pair<Request, Response, RpcImpl>(
    rpc_impl: RpcImpl
) -> (impl RpcClient<Request, Response>, impl RpcServer)
where
    Request: Send + 'static,
    Response: Send + 'static,
    RpcImpl: Rpc<Request, Response>,
{
    let (client_send, server_recv) = unbounded_channel::<Request>();
    let (server_send, client_recv) = unbounded_channel::<Response>();

    let client = LocalRpcClient {
        send: client_send,
        recv: client_recv,
    };

    let server = LocalRpcServer {
        send: server_send,
        recv: server_recv,
        rpc_impl,
    };

    (client, server)
}

// LOCAL SERVER
// --------------------------------------------------------------------------------------

struct LocalRpcServer<Request, Response, S>
where
    Request: Send + 'static,
    Response: Send + 'static,
    S: Rpc<Request, Response>,
{
    send: UnboundedSender<Response>,
    recv: UnboundedReceiver<Request>,
    rpc_impl: S,
}

#[async_trait]
impl<T, U, S> RpcServer for LocalRpcServer<T, U, S>
where
    T: Send + 'static,
    U: Send + 'static,
    S: Rpc<T, U>,
{
    async fn serve(mut self) -> Result<(), RpcError> {
        loop {
            let request = self.recv.recv().await.ok_or(RpcError::RecvError)?;
            let response = self.rpc_impl.handle_request(request).await;
            self.send.send(response)?;
        }
    }
}

// LOCAL CLIENT
// --------------------------------------------------------------------------------------

struct LocalRpcClient<Request, Response>
where
    Request: Send + 'static,
    Response: Send + 'static,
{
    send: UnboundedSender<Request>,
    recv: UnboundedReceiver<Response>,
}

#[async_trait]
impl<Request, Response> RpcClient<Request, Response> for LocalRpcClient<Request, Response>
where
    Request: Send + 'static,
    Response: Send + 'static,
{
    async fn call(
        &mut self,
        x: Request,
    ) -> Result<Response, RpcError> {
        self.send.send(x)?;
        self.recv.recv().await.ok_or(RpcError::RecvError)
    }
}

// MISC
// --------------------------------------------------------------------------------------

impl<T> From<SendError<T>> for RpcError {
    fn from(_send_error: SendError<T>) -> Self {
        Self::SendError
    }
}
