//! Utility wrapper around [tokio::sync::mpsc] channels, enabling multiple
//! producers to send messages to a single subscriber, with each message
//! able to be responded to by the subscriber.

use core::{fmt, result::Result};
use tokio;
use tokio::sync::{mpsc, mpsc::error::SendError, oneshot, oneshot::error::RecvError};

impl std::error::Error for ChannelError {}
impl fmt::Display for ChannelError {
    fn fmt(&self, fmt: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChannelError::SendError => write!(fmt, "channel send error"),
            ChannelError::RecvError => write!(fmt, "channel receiver error"),
        }
    }
}
/// Error type to wrap the potential tokio sync errors,
/// and distinguish between user-land respond errors.
#[derive(Debug)]
pub enum ChannelError {
    /// An occurred during sending a message.
    /// From [tokio::sync::mpsc::error::SendError].
    SendError,
    /// An occurred during the receiving of a message.
    /// From [tokio::sync::mpsc::error::RecvError].
    RecvError,
}

impl<Q, S, E> From<SendError<Message<Q, S, E>>> for ChannelError {
    fn from(_: SendError<Message<Q, S, E>>) -> Self {
        ChannelError::SendError
    }
}

impl From<RecvError> for ChannelError {
    fn from(_: RecvError) -> Self {
        ChannelError::RecvError
    }
}

/// Represents a request to be processed in [MessageProcessor],
/// sent from the associated [MessageClient].
pub struct Message<Q, S, E> {
    /// The initial request the [Message] is wrapping.
    pub request: Q,
    sender: oneshot::Sender<Result<S, E>>,
}

impl<Q, S, E> Message<Q, S, E> {
    /// Send `response` to the originator of this [Message].
    /// Each message can only be responded to once.
    pub fn respond(self, response: Result<S, E>) -> bool {
        self.sender.send(response).map_or_else(|_| false, |_| true)
    }
}

impl<Q: std::fmt::Debug, S, E> fmt::Debug for Message<Q, S, E> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Message")
            .field("request", &self.request)
            .finish()
    }
}

/// Sends requests to the associated `MessageProcessor`.
///
/// Instances are created by the
/// [`message_channel`](message_channel) function.
#[derive(Debug)]
pub struct MessageClient<Q, S, E> {
    tx: mpsc::UnboundedSender<Message<Q, S, E>>,
}

impl<Q, S, E> MessageClient<Q, S, E> {
    /// Sends a one-way request to the corresponding receiver. Use
    /// [MessageClient::send] if the receiver should be able to respond.
    #[allow(dead_code)]
    pub fn send_oneshot(&self, request: Q) -> Result<(), ChannelError> {
        self.send_request_impl(request)
            .map(|_| Ok(()))
            .map_err(ChannelError::from)?
    }

    /// Sends a request to the corresponding receiver where it can be
    /// responded to.
    pub async fn send(&self, request: Q) -> Result<Result<S, E>, ChannelError> {
        let rx = self
            .send_request_impl(request)
            .map_err(ChannelError::from)?;
        rx.await.map_err(|e| e.into())
    }

    #[allow(clippy::type_complexity)]
    fn send_request_impl(
        &self,
        request: Q,
    ) -> Result<oneshot::Receiver<Result<S, E>>, SendError<Message<Q, S, E>>> {
        let (tx, rx) = oneshot::channel::<Result<S, E>>();
        let message = Message {
            sender: tx,
            request,
        };

        self.tx.send(message).map(|_| rx)
    }
}

// Manually implement `Clone` so that the generics do not need
// also implement.
impl<Q, S, E> Clone for MessageClient<Q, S, E> {
    fn clone(&self) -> Self {
        MessageClient {
            tx: self.tx.clone(),
        }
    }
}

/// Receives requests from the associated `MessageClient`,
/// and optionally sends a response.
///
/// Instances are created by the [message_channel] function.
pub struct MessageProcessor<Q, S, E> {
    rx: mpsc::UnboundedReceiver<Message<Q, S, E>>,
}

impl<Q, S, E> MessageProcessor<Q, S, E> {
    /// Awaits until it can return a new message to process, or
    /// [None] if all senders have been terminated.
    pub async fn pull_message(&mut self) -> Option<Message<Q, S, E>> {
        self.rx.recv().await
    }
}

/// Creates a pair of bound `MessageClient` and `MessageProcessor`.
pub fn message_channel<Q, S, E>() -> (MessageClient<Q, S, E>, MessageProcessor<Q, S, E>) {
    let (tx, rx) = mpsc::unbounded_channel::<Message<Q, S, E>>();
    let processor = MessageProcessor::<Q, S, E> { rx };
    let client = MessageClient::<Q, S, E> { tx };
    (client, processor)
}

#[cfg(test)]
mod tests {
    enum Request {
        Ping(),
        SetFlag(u32),
        Shutdown(),
        Throw(),
    }

    enum Response {
        Pong(),
        GenericResult(bool),
    }
    struct TestError {
        pub message: String,
    }
    use super::*;
    #[tokio::test]
    async fn test_message_channel() -> Result<(), Box<dyn std::error::Error>> {
        let (client, mut processor) = message_channel::<Request, Response, TestError>();

        tokio::spawn(async move {
            let mut set_flags: usize = 0;

            loop {
                let message = processor.pull_message().await;
                match message {
                    Some(m) => match m.request {
                        Request::Ping() => {
                            let success = m.respond(Ok(Response::Pong()));
                            assert!(success, "receiver not closed");
                        }
                        Request::Throw() => {
                            m.respond(Err(TestError {
                                message: String::from("thrown!"),
                            }));
                        }
                        Request::SetFlag(_) => {
                            set_flags += 1;
                            let success = m.respond(Ok(Response::GenericResult(true)));
                            assert!(
                                !success,
                                "one-way requests should not successfully respond."
                            );
                        }
                        Request::Shutdown() => {
                            assert_eq!(set_flags, 10, "One-way requests successfully processed.");
                            let success = m.respond(Ok(Response::GenericResult(true)));
                            assert!(success);
                            return;
                        }
                    },
                    None => panic!("message queue empty"),
                }
            }
        });

        let res = client.send(Request::Ping()).await?;
        matches!(res, Ok(Response::Pong()));

        for n in 0..10 {
            client.send_oneshot(Request::SetFlag(n))?;
        }

        let res = client.send(Request::Throw()).await?;
        assert!(
            match res {
                Ok(_) => false,
                Err(TestError { message }) => {
                    assert_eq!(message, String::from("thrown!"));
                    true
                }
            },
            "User Error propagates to client."
        );

        let res = client.send(Request::Shutdown()).await?;
        assert!(
            match res {
                Ok(Response::GenericResult(success)) => success,
                _ => false,
            },
            "successfully shutdown processing thread."
        );

        Ok(())
    }
}
