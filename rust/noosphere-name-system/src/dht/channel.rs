use anyhow::{anyhow, Result};
use tokio;
use tokio::sync::{mpsc, oneshot};

/// Represents a request to be processed in `MessageProcessor`,
/// sent from the associated `MessageClient`.
pub struct Message<Q, S> {
    pub request: Q,
    sender: oneshot::Sender<Result<S>>,
}

impl<Q, S> Message<Q, S> {
    pub fn respond(self, response: Result<S>) -> bool {
        self.sender.send(response).map_or_else(|_| false, |_| true)
    }
}

/// Sends requests to the associated `MessageProcessor`.
///
/// Instances are created by the
/// [`message_channel`](message_channel) function.
pub struct MessageClient<Q, S> {
    tx: mpsc::UnboundedSender<Message<Q, S>>,
}

impl<Q, S> MessageClient<Q, S> {
    // TBD if/how "synchronous" requests will work.
    #[allow(dead_code)]
    pub fn send_request(&self, request: Q) -> Result<()> {
        self.send_request_impl(request).map(|_| Ok(()))?
    }

    pub async fn send_request_async(&self, request: Q) -> Result<S> {
        let rx = self.send_request_impl(request)?;
        let outer_result = rx.await;
        // Unwrap the outer Result, potentially containing communication
        // errors (RecvError), like the sender prematurely dropping
        // the connection.
        let inner_result = outer_result.map_err(|e| anyhow!(e.to_string()))?;
        inner_result
    }

    fn send_request_impl(&self, request: Q) -> Result<oneshot::Receiver<Result<S>>> {
        let (tx, rx) = oneshot::channel::<Result<S>>();
        let message = Message {
            sender: tx,
            request,
        };

        if let Err(e) = self.tx.send(message) {
            return Err(anyhow!(e.to_string()));
        }
        Ok(rx)
    }
}

/// Receives requests from the associated `MessageClient`,
/// and optionally sends a response.
///
/// Instances are created by the
/// [`message_channel`](message_channel) function.
pub struct MessageProcessor<Q, S> {
    rx: mpsc::UnboundedReceiver<Message<Q, S>>,
}

impl<Q, S> MessageProcessor<Q, S> {
    pub async fn pull_message(&mut self) -> Option<Message<Q, S>> {
        self.rx.recv().await
    }
}

/// Creates a pair of bound `MessageClient` and `MessageProcessor`.
pub fn message_channel<Q, S>() -> (MessageClient<Q, S>, MessageProcessor<Q, S>) {
    let (tx, rx) = mpsc::unbounded_channel::<Message<Q, S>>();
    let processor = MessageProcessor::<Q, S> { rx };
    let client = MessageClient::<Q, S> { tx };
    (client, processor)
}

#[cfg(test)]
mod tests {
    pub enum Request {
        Ping(),
        SetFlag(u32),
        Shutdown(),
        Throw(),
    }

    pub enum Response {
        Pong(),
        GenericResult(bool),
    }
    use super::*;
    #[tokio::test]
    async fn test_message_channel() -> Result<()> {
        let (mut client, mut processor) = message_channel();

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
                            m.respond(Err(anyhow!("MyError!")));
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
                        _ => panic!("no handler for request"),
                    },
                    None => panic!("message queue empty"),
                }
            }
        });

        let res = client.send_request_async(Request::Ping()).await?;
        assert!(match res {
            Response::Pong() => true,
            _ => false,
        });

        for n in 0..10 {
            client.send_request(Request::SetFlag(n))?;
        }

        let res = client.send_request_async(Request::Throw()).await;
        assert!(res.is_err(), "Error propagates to client.");

        let res = client.send_request_async(Request::Shutdown()).await?;
        assert!(
            match res {
                Response::GenericResult(success) => success,
                _ => false,
            },
            "successfully shutdown processing thread."
        );

        Ok(())
    }
}
