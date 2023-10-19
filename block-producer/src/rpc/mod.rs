use std::{future::Future, sync::Arc};

use async_trait::async_trait;
use tokio::sync::{
    mpsc::{error::SendError, unbounded_channel, UnboundedReceiver, UnboundedSender},
    oneshot,
};

/// Creates a client/server pair that communicate locally using channels
pub fn create_client_server_pair<Request, Response, S>(
    server_impl: S
) -> (RpcClient<Request, Response>, RpcServer<Request, Response, S>)
where
    Request: Send + 'static,
    Response: Send + 'static,
    S: ServerImpl<Request, Response>,
{
    let (sender, recv) = unbounded_channel::<(Request, oneshot::Sender<Response>)>();

    let client = RpcClient {
        send_requests: sender,
    };

    let server = RpcServer {
        recv_requests: recv,
        server_impl: Arc::new(server_impl),
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

/// Implements the processing of an RPC request on the server.
/// Every request is processed in a new task.
#[async_trait]
pub trait ServerImpl<Request, Response>: Send + Sync + 'static {
    async fn handle_request(
        self: Arc<Self>,
        request: Request,
    ) -> Response;
}

// RPC SERVER
// --------------------------------------------------------------------------------------

pub struct RpcServer<Request, Response, S>
where
    Request: Send + 'static,
    Response: Send + 'static,
    S: ServerImpl<Request, Response>,
{
    recv_requests: UnboundedReceiver<(Request, oneshot::Sender<Response>)>,
    server_impl: Arc<S>,
}

impl<Request, Response, S> RpcServer<Request, Response, S>
where
    Request: Send + 'static,
    Response: Send + 'static,
    S: ServerImpl<Request, Response>,
{
    pub async fn serve(mut self) -> Result<(), RpcError> {
        loop {
            let (request, response_channel) =
                self.recv_requests.recv().await.ok_or(RpcError::RecvError).expect("rpc server");

            let server_impl = self.server_impl.clone();
            tokio::spawn(async move {
                let response = server_impl.handle_request(request).await;
                let _ = response_channel.send(response);
            });
        }
    }
}

// RPC CLIENT
// --------------------------------------------------------------------------------------

#[derive(Clone)]
pub struct RpcClient<Request, Response>
where
    Request: Send + 'static,
    Response: Send + 'static,
{
    send_requests: UnboundedSender<(Request, oneshot::Sender<Response>)>,
}

impl<Request, Response> RpcClient<Request, Response>
where
    Request: Send + 'static,
    Response: Send + 'static,
{
    pub fn call(
        &self,
        req: Request,
    ) -> Result<impl Future<Output = Result<Response, RpcError>>, RpcError> {
        let (sender, rx) = oneshot::channel();
        self.send_requests.send((req, sender))?;

        Ok(async move {
            let response = rx.await.map_err(|_| RpcError::RecvError)?;
            Ok(response)
        })
    }
}
