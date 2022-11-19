///! This task manager is not currently used, but a potential
///! way of managing IPFS/NS tasks with retry, exponential backoffs,
///! and resumeability.
use async_trait::async_trait;
use futures::stream::{self, StreamExt};
use std::marker::Send;
use tokio;
use tokio::{
    spawn,
    sync::{mpsc, mpsc::error::SendError, oneshot, oneshot::error::RecvError},
    task::JoinError,
};

/// An interface for submitting tasks to be processed
/// amongst a pool of threads.
pub struct TaskRunner<T: Task> {
    thread: Thread<ManagerThread<T>>,
}

impl<T: Task> TaskRunner<T> {
    /// Create a [TaskRunner] with `pool_size` threads.
    pub fn new(pool_size: u8) -> Self {
        Self {
            thread: Thread::new(ManagerThread::new(pool_size)),
        }
    }

    /// Submit [Task] to be processed.
    pub fn post_task(&self, task: T) {
        self.thread.send(task);
    }
}

pub enum TaskError {
    NoChannel,
}

pub trait StaticSendable: Send + 'static {}
impl<T: Send + 'static> StaticSendable for T {}

/// An object implementing [Task] describes a process
/// to run on a worker thread.
#[async_trait]
pub trait Task: StaticSendable {
    async fn run(&self) -> Result<(), TaskError>;
}

#[async_trait]
trait ThreadProcessor: StaticSendable {
    type Request;
    type Response;
    fn connect(
        &mut self,
        rx: ThreadProcessorReceiver<Self::Request>,
        tx: ThreadProcessorSender<Self::Response>,
    );
    async fn process(&mut self) -> Result<(), TaskError>;
}
type ThreadProcessorReceiver<T> = mpsc::UnboundedReceiver<T>;
type ThreadProcessorSender<T> = mpsc::UnboundedSender<T>;

/// [Thread] represents an underlying child thread running the
/// provided [ThreadProcessor]. The child thread's lifetime is mapped
/// to its [Thread] lifetime, i.e. drop [Thread] to kill the child thread.
/// The child thread can send/receive messages via `send()` and `recv()`.
struct Thread<T: ThreadProcessor> {
    tx: ThreadProcessorSender<T::Request>,
    rx: Option<ThreadProcessorReceiver<T::Response>>,
    handle: Option<tokio::task::JoinHandle<Result<(), TaskError>>>,
}

impl<T: ThreadProcessor> Thread<T> {
    fn new(mut processor: T) -> Self {
        let (tx1, rx1) = mpsc::unbounded_channel::<T::Request>();
        let (tx2, rx2) = mpsc::unbounded_channel::<T::Response>();
        processor.connect(rx1, tx2);
        let handle = Some(spawn(async move { processor.process().await }));
        Thread {
            tx: tx1,
            rx: Some(rx2),
            handle,
        }
    }

    fn new_with_mpsc_response(
        mut processor: T,
        mpsc_sender: ThreadProcessorSender<T::Response>,
    ) -> Self {
        let (tx1, rx1) = mpsc::unbounded_channel::<T::Request>();
        processor.connect(rx1, mpsc_sender);
        let handle = Some(spawn(async move { processor.process().await }));
        Thread {
            tx: tx1,
            rx: None,
            handle,
        }
    }

    fn send(&self, req: T::Request) {
        self.tx.send(req);
    }

    async fn recv(&mut self) -> Option<T::Response> {
        if let Some(rx) = self.rx.as_mut() {
            rx.recv().await
        } else {
            None
        }
    }
}

impl<T: ThreadProcessor> Drop for Thread<T> {
    fn drop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
    }
}

pub struct ManagerThread<T: Task> {
    pool_size: u8,
    rx: Option<ThreadProcessorReceiver<T>>,
    tx: Option<ThreadProcessorSender<T>>,
}

impl<T: Task> ManagerThread<T> {
    pub fn new(pool_size: u8) -> Self {
        ManagerThread {
            pool_size,
            rx: None,
            tx: None,
        }
    }
}

#[async_trait]
impl<T: Task> ThreadProcessor for ManagerThread<T> {
    type Request = T;
    type Response = T;
    fn connect(
        &mut self,
        rx: ThreadProcessorReceiver<Self::Request>,
        tx: ThreadProcessorSender<Self::Response>,
    ) {
        self.rx = Some(rx);
        self.tx = Some(tx);
    }

    async fn process(&mut self) -> Result<(), TaskError> {
        let (worker_tx, mut worker_rx) =
            mpsc::unbounded_channel::<<WorkerThread<T> as ThreadProcessor>::Response>();
        let mut thread_handles = vec![];
        for _ in 0..self.pool_size {
            thread_handles.push(Thread::new_with_mpsc_response(
                WorkerThread::new(),
                worker_tx.clone(),
            ));
        }

        let mut rx = self.rx.take().ok_or(TaskError::NoChannel)?;

        tokio::select! {
            Some(response) = worker_rx.recv() => {

            }
            Some(task) = rx.recv() => {
                thread_handles.get(0).unwrap().send(TaskRequest { task: Some(task) });
            }
        };

        Ok(())
    }
}

struct WorkerThread<T: Task> {
    rx: Option<ThreadProcessorReceiver<TaskRequest<T>>>,
    tx: Option<ThreadProcessorSender<TaskResponse<T>>>,
}

impl<T: Task> WorkerThread<T> {
    fn new() -> Self {
        Self { rx: None, tx: None }
    }
}

#[async_trait]
impl<T: Task> ThreadProcessor for WorkerThread<T> {
    type Request = TaskRequest<T>;
    type Response = TaskResponse<T>;
    fn connect(
        &mut self,
        rx: ThreadProcessorReceiver<Self::Request>,
        tx: ThreadProcessorSender<Self::Response>,
    ) {
        self.rx = Some(rx);
        self.tx = Some(tx);
    }

    async fn process(&mut self) -> Result<(), TaskError> {
        let tx = self.tx.take().ok_or(TaskError::NoChannel)?;
        let mut rx = self.rx.take().ok_or(TaskError::NoChannel)?;

        loop {
            tokio::select! {
              Some(mut task_request) = rx.recv() => {
                let task = match task_request.try_take_task() {
                    Ok(task) => task,
                    Err(_) => {
                        // @TODO Handle task error
                        break;
                    }
                };

                let result = {
                    let t = &task;
                    t.run().await
                };
                /*match result {
                    Err(e) => {}
                    Ok(_) => {
                        tx.send(TaskResponse::<T> { task });
                    }
                };
                */
            }}
        }
        Ok(())
    }
}

struct TaskRequest<T: Task> {
    task: Option<T>,
}

impl<T: Task> TaskRequest<T> {
    pub fn new(task: T) -> Self {
        Self { task: Some(task) }
    }

    pub fn try_take_task(&mut self) -> Result<T, ()> {
        self.task.take().ok_or_else(|| ())
    }
}

struct TaskResponse<T: Task> {
    task: T,
}

#[cfg(test)]
mod test {
    use super::*;
    use std::sync::Arc;
    use std::thread::{self, ThreadId};
    use std::time::Duration;
    use tokio::sync::Mutex as TokioMutex;
    use tokio::time::sleep;

    struct DebugTaskState {
        id: u8,
        thread_id: ThreadId,
    }

    struct DebugTask {
        id: u8,
        wait: u64,
        state: Arc<TokioMutex<Vec<DebugTaskState>>>,
    }

    #[async_trait]
    impl Task for DebugTask {
        async fn run(&self) -> Result<(), TaskError> {
            sleep(Duration::from_millis(self.wait)).await;
            let thread_id = thread::current().id();
            let mut state = self.state.lock().await;
            state.push(DebugTaskState {
                id: self.id,
                thread_id,
            });
            Ok(())
        }
    }

    #[tokio::test]
    async fn it_queues_tasks() {
        let state = Arc::new(TokioMutex::new(0u32));

        struct AddTask {
            value: u32,
            state: Arc<TokioMutex<u32>>,
        }

        #[async_trait]
        impl Task for AddTask {
            async fn run(&self) -> Result<(), TaskError> {
                let mut total = self.state.lock().await;
                *total += self.value;
                Ok(())
            }
        }

        let mut task_queue = TaskRunner::new(1);
        for i in 1..10 {
            task_queue.post_task(AddTask {
                value: i,
                state: Arc::clone(&state),
            })
        }

        while *state.lock().await != 45 {
            sleep(Duration::from_millis(5)).await;
        }

        assert_eq!(*state.lock().await, 45);
    }

    #[tokio::test]
    async fn it_distributes_work_amongst_thread_pool() {
        let state: Arc<TokioMutex<Vec<DebugTaskState>>> = Arc::new(TokioMutex::new(vec![]));
        let thread_count = 4;
        let runner = TaskRunner::new(thread_count);

        for i in 0..10 {
            let wait = if i == 0 { 100 } else { 0 };
            runner.post_task(DebugTask {
                id: i,
                wait,
                state: Arc::clone(&state),
            })
        }

        while (state.lock().await).len() != 10 {
            sleep(Duration::from_millis(5)).await;
        }

        let state = state.lock().await;
        assert_eq!(state.len(), 10);

        let mut thread_ids: Vec<ThreadId> = vec![];
        for (i, result) in state.iter().enumerate() {
            if !thread_ids.contains(&result.thread_id) {
                thread_ids.push(result.thread_id.clone());
            }

            if i == 0 {
                assert!(
                    result.id != 0,
                    "slow task, though submitted first, should not complete first."
                );
            }
        }
        assert_eq!(
            thread_ids.len(),
            thread_count as usize,
            "all worker threads should be used."
        );
    }
}
