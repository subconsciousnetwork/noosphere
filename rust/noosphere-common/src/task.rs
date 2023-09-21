use anyhow::Result;
use std::future::Future;

#[cfg(target_arch = "wasm32")]
use std::pin::Pin;

#[cfg(target_arch = "wasm32")]
use tokio::sync::oneshot::channel;

#[cfg(target_arch = "wasm32")]
use futures::future::join_all;

#[cfg(not(target_arch = "wasm32"))]
use tokio::task::JoinSet;

use crate::ConditionalSend;

#[cfg(target_arch = "wasm32")]
/// Spawn a future by scheduling it with the local executor. The returned
/// future will be pending until the spawned future completes.
pub async fn spawn<F>(future: F) -> Result<F::Output>
where
    F: Future + 'static,
    F::Output: Send + 'static,
{
    let (tx, rx) = channel();

    wasm_bindgen_futures::spawn_local(async move {
        if let Err(_) = tx.send(future.await) {
            warn!("Receiver dropped before spawned task completed");
        }
    });

    Ok(rx.await?)
}

#[cfg(not(target_arch = "wasm32"))]
/// Spawn a future by scheduling it with the local executor. The returned
/// future will be pending until the spawned future completes.
pub async fn spawn<F>(future: F) -> Result<F::Output>
where
    F: Future + Send + 'static,
    F::Output: Send + 'static,
{
    Ok(tokio::spawn(future).await?)
}

/// Spawns a [ConditionalSend] future without requiring `await`.
/// The future will immediately start processing.
pub fn spawn_no_wait<F>(future: F)
where
    F: Future<Output = ()> + ConditionalSend + 'static,
{
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_futures::spawn_local(future);
    #[cfg(not(target_arch = "wasm32"))]
    tokio::task::spawn(future);
}

/// An aggregator of async work that can be used to observe the moment when all
/// the aggregated work is completed. It is similar to tokio's [JoinSet], but is
/// relatively constrained and also works on `wasm32-unknown-unknown`. Unlike
/// [JoinSet], the results can not be observed individually.
///
/// ```rust
/// # use anyhow::Result;
/// # use noosphere_common::TaskQueue;
/// #
/// # #[tokio::main(flavor = "multi_thread")]
/// # async fn main() -> Result<()> {
/// #
/// let mut task_queue = TaskQueue::default();
/// for i in 0..10 {
///     task_queue.spawn(async move {
///         println!("{}", i);
///         Ok(())
///     });
/// }
/// task_queue.join().await?;
/// #
/// #   Ok(())
/// # }
/// ```
#[derive(Default)]
pub struct TaskQueue {
    #[cfg(not(target_arch = "wasm32"))]
    tasks: JoinSet<Result<()>>,

    #[cfg(target_arch = "wasm32")]
    tasks: Vec<Pin<Box<dyn Future<Output = ()>>>>,
}

impl TaskQueue {
    #[cfg(not(target_arch = "wasm32"))]
    /// Queue a future to be spawned in the local executor. All queued futures will be polled
    /// to completion before the [TaskQueue] can be joined.
    pub fn spawn<F>(&mut self, future: F)
    where
        F: Future<Output = Result<()>> + Send + 'static,
    {
        self.tasks.spawn(future);
    }

    #[cfg(not(target_arch = "wasm32"))]
    /// Returns a future that finishes when all queued futures have finished.
    pub async fn join(&mut self) -> Result<()> {
        while let Some(result) = self.tasks.join_next().await {
            trace!("Task completed, {} remaining in queue...", self.tasks.len());
            result??;
        }
        Ok(())
    }

    #[cfg(target_arch = "wasm32")]
    /// Queue a future to be spawned in the local executor. All queued futures will be polled
    /// to completion before the [TaskQueue] can be joined.
    pub fn spawn<F>(&mut self, future: F)
    where
        F: Future<Output = Result<()>> + 'static,
    {
        let task_count = self.tasks.len();

        self.tasks.push(Box::pin(async move {
            if let Err(error) = spawn(future).await {
                error!("Queued task failed: {:?}", error);
            }
            trace!("Task {} completed...", task_count + 1);
        }));
    }

    #[cfg(target_arch = "wasm32")]
    /// Returns a future that finishes when all queued futures have finished.
    pub async fn join(&mut self) -> Result<()> {
        let tasks = std::mem::replace(&mut self.tasks, Vec::new());

        debug!("Joining {} queued tasks...", tasks.len());

        join_all(tasks).await;

        Ok(())
    }
}
