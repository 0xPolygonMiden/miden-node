use std::{future::Future, sync::Arc};

use async_trait::async_trait;
use tokio::sync::{
    mpsc::{error::SendError, unbounded_channel, UnboundedReceiver, UnboundedSender},
    oneshot,
};

/// Creates a sender/receiver pair that communicate locally using channels
pub fn create_message_sender_receiver_pair<Message, Response, Handler>(
    message_handler: Arc<Handler>
) -> (MessageSender<Message, Response>, MessageReceiver<Message, Response, Handler>)
where
    Message: Send + 'static,
    Response: Send + 'static,
    Handler: MessageHandler<Message, Response>,
{
    let (sender, recv) = unbounded_channel::<(Message, oneshot::Sender<Response>)>();

    let client = MessageSender {
        send_messages: sender,
    };

    let server = MessageReceiver {
        recv_messages: recv,
        handler: message_handler,
    };

    (client, server)
}

/// Errors related to the message mechanism itself
/// TODO: Make errors more descriptive
#[derive(Debug)]
pub enum MessageError {
    SendError,
    RecvError,
}

impl<T> From<SendError<T>> for MessageError {
    fn from(_send_error: SendError<T>) -> Self {
        Self::SendError
    }
}

/// Implements the processing of a message on the [`MessageReceiver`].
/// Every message is processed in a new task.
#[async_trait]
pub trait MessageHandler<Message, Response>: Send + Sync + 'static {
    async fn handle_message(
        self: Arc<Self>,
        message: Message,
    ) -> Response;
}

// MESSAGE RECEIVER
// --------------------------------------------------------------------------------------

pub struct MessageReceiver<Message, Response, Handler>
where
    Message: Send + 'static,
    Response: Send + 'static,
    Handler: MessageHandler<Message, Response>,
{
    recv_messages: UnboundedReceiver<(Message, oneshot::Sender<Response>)>,
    handler: Arc<Handler>,
}

impl<Message, Response, Handler> MessageReceiver<Message, Response, Handler>
where
    Message: Send + 'static,
    Response: Send + 'static,
    Handler: MessageHandler<Message, Response>,
{
    pub async fn serve(mut self) -> Result<(), MessageError> {
        loop {
            let (message, response_channel) = self
                .recv_messages
                .recv()
                .await
                .ok_or(MessageError::RecvError)
                .expect("rpc server");

            let message_handler = self.handler.clone();
            tokio::spawn(async move {
                let response = message_handler.handle_message(message).await;
                let _ = response_channel.send(response);
            });
        }
    }
}

// MESSAGE SENDER
// --------------------------------------------------------------------------------------

#[derive(Clone)]
pub struct MessageSender<Message, Response>
where
    Message: Send + 'static,
    Response: Send + 'static,
{
    send_messages: UnboundedSender<(Message, oneshot::Sender<Response>)>,
}

impl<Message, Response> MessageSender<Message, Response>
where
    Message: Send + 'static,
    Response: Send + 'static,
{
    pub fn call(
        &self,
        req: Message,
    ) -> Result<impl Future<Output = Result<Response, MessageError>>, MessageError> {
        let (sender, rx) = oneshot::channel();
        self.send_messages.send((req, sender))?;

        Ok(async move {
            let response = rx.await.map_err(|_| MessageError::RecvError)?;
            Ok(response)
        })
    }
}
