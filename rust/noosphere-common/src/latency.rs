use instant::{Duration, Instant};

use futures_util::Stream;
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};

use crate::ConditionalSend;

/// A helper for observing when [Stream] throughput appears to have stalled
pub struct StreamLatencyGuard<S>
where
    S: Stream + Unpin,
    S::Item: ConditionalSend + 'static,
{
    inner: S,
    threshold: Duration,
    last_ready_time: Instant,
    tx: UnboundedSender<()>,
}

impl<S> StreamLatencyGuard<S>
where
    S: Stream + Unpin,
    S::Item: ConditionalSend + 'static,
{
    /// Wraps a [Stream] and provides an [UnboundedReceiver<()>] that will receive
    /// a message any time the wrapped [Stream] is pending for longer than the provided
    /// threshold [Duration].
    pub fn wrap(stream: S, threshold: Duration) -> (Self, UnboundedReceiver<()>) {
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel::<()>();
        (
            StreamLatencyGuard {
                inner: stream,
                threshold,
                last_ready_time: Instant::now(),
                tx,
            },
            rx,
        )
    }
}

impl<S> Stream for StreamLatencyGuard<S>
where
    S: Stream + Unpin,
    S::Item: ConditionalSend + 'static,
{
    type Item = S::Item;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let result = std::pin::pin!(&mut self.inner).poll_next(cx);

        if result.is_pending() {
            if Instant::now() - self.last_ready_time > self.threshold {
                let _ = self.tx.send(());
            }
        } else if result.is_ready() {
            self.last_ready_time = Instant::now();
        }

        result
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use instant::Duration;
    use tokio::select;
    use tokio_stream::StreamExt;

    use crate::{helpers::wait, StreamLatencyGuard};

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_does_not_impede_the_behavior_of_a_wrapped_stream() -> Result<()> {
        let stream = tokio_stream::iter(Vec::from([0u32; 1024]));

        let (guarded_stream, _latency_signal) =
            StreamLatencyGuard::wrap(stream, Duration::from_secs(1));

        tokio::pin!(guarded_stream);

        guarded_stream.collect::<Vec<u32>>().await;

        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_signals_when_a_stream_encounters_latency() -> Result<()> {
        let stream = Box::pin(futures_util::stream::unfold(0, |index| async move {
            match index {
                512 => {
                    for _ in 0..3 {
                        // Uh oh, latency! Note that `tokio::time::sleep` is observed to cooperate
                        // with the runtime, so we wait multiple times to ensure that the stream is
                        // actually polled multiple times
                        wait(1).await;
                    }
                    Some((index, index + 1))
                }
                _ if index < 1024 => Some((index, index + 1)),
                _ => None,
            }
        }));

        let (guarded_stream, mut latency_guard) =
            StreamLatencyGuard::wrap(stream, Duration::from_millis(100));

        tokio::pin!(guarded_stream);

        select! {
            _ = guarded_stream.collect::<Vec<u32>>() => {
                unreachable!("Latency guard should be hit first");
            },
            _ = latency_guard.recv() => ()
        }

        Ok(())
    }
}
