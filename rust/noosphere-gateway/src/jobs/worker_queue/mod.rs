//! Contains a generic worker queue in service of Noosphere
//! Gateway job processing.

mod builder;
mod processor;
mod queue;
mod queue_thread;
mod worker;

pub use builder::*;
pub use processor::*;
pub use queue::*;

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::{anyhow, Result};
    use async_trait::async_trait;
    use std::{sync::Arc, time::Duration};
    use tokio::sync::Mutex;

    #[derive(Clone, Debug)]
    enum TestJob {
        Ping(String),
        Sleep(u64),
        QueuePing(String),
        WillFail(String),
    }

    impl std::fmt::Display for TestJob {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            match self {
                TestJob::Ping(s) => write!(f, "Ping({})", s),
                TestJob::Sleep(s) => write!(f, "Sleep({})", s),
                TestJob::QueuePing(s) => write!(f, "QueuePing({})", s),
                TestJob::WillFail(s) => write!(f, "WillFail({})", s),
            }
        }
    }

    #[derive(Clone)]
    struct TestProcessor {}
    #[async_trait]
    impl Processor for TestProcessor {
        type Context = Arc<Mutex<Vec<String>>>;
        type Job = TestJob;
        async fn process(context: Self::Context, job: Self::Job) -> Result<Option<Self::Job>> {
            {
                let mut ctx = context.lock().await;
                ctx.push(job.to_string());
            }

            let result = match job {
                TestJob::Ping(_) => Ok(None),
                TestJob::Sleep(seconds) => {
                    tokio::time::sleep(Duration::from_secs(seconds)).await;
                    Ok(None)
                }
                TestJob::QueuePing(s) => Ok(Some(TestJob::Ping(s))),
                TestJob::WillFail(s) => Err(anyhow!("WillFail({}) has failed!!", s)),
            };

            result
        }
    }

    /// Checks context and waits for all expected jobs to finish, and asserts
    /// all expected jobs have been found in the context, our indication
    /// a job has been processed.
    async fn assert_context(context: Arc<Mutex<Vec<String>>>, expected_jobs: &Vec<TestJob>) {
        let expected_len = expected_jobs.len();
        loop {
            tokio::time::sleep(Duration::from_secs(1)).await;
            let ctx = context.lock().await;
            if ctx.len() == expected_len {
                break;
            }
        }

        let ctx = context.lock().await;
        assert_eq!(ctx.len(), expected_len);
        for expected_job in expected_jobs {
            assert!(ctx
                .iter()
                .find(|x| x.as_str() == expected_job.to_string())
                .is_some());
        }
    }

    #[tokio::test]
    async fn test_worker_queue_simple() -> Result<()> {
        let context: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(vec![]));

        let queue = WorkerQueueBuilder::<TestProcessor>::new()
            .with_worker_count(2)
            .with_timeout(Duration::from_secs(9999))
            .with_context(context.clone())
            .build()?;

        let jobs = vec![
            TestJob::Sleep(1),
            TestJob::Ping("Hello".into()),
            TestJob::Ping("World".into()),
            TestJob::Ping("It's".into()),
            TestJob::Ping("Noosphere".into()),
        ];
        for job in &jobs {
            queue.submit(job.to_owned())?;
        }

        assert_context(context, &jobs).await;
        Ok(())
    }

    #[tokio::test]
    async fn test_worker_queue_subsequent_job() -> Result<()> {
        let context: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(vec![]));

        let queue = WorkerQueueBuilder::<TestProcessor>::new()
            .with_worker_count(2)
            .with_timeout(Duration::from_secs(9999))
            .with_context(context.clone())
            .build()?;

        let mut jobs = vec![
            TestJob::QueuePing("LatePing1".into()),
            TestJob::QueuePing("LatePing2".into()),
        ];

        for job in &jobs {
            queue.submit(job.to_owned())?;
        }

        // We expect `Ping` jobs to be queued up after `QueuePing` jobs.
        jobs.push(TestJob::Ping("LatePing1".into()));
        jobs.push(TestJob::Ping("LatePing2".into()));

        assert_context(context, &jobs).await;
        Ok(())
    }

    #[tokio::test]
    async fn test_worker_queue_retries() -> Result<()> {
        let context: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(vec![]));

        let queue = WorkerQueueBuilder::<TestProcessor>::new()
            .with_worker_count(1)
            .with_timeout(Duration::from_secs(9999))
            .with_retries(3)
            .with_context(context.clone())
            .build()?;

        let mut jobs = vec![
            TestJob::WillFail("expectedfailure".into()),
            TestJob::Ping("ping1".into()),
            TestJob::Ping("ping2".into()),
            TestJob::Ping("ping3".into()),
        ];

        for job in &jobs {
            queue.submit(job.to_owned())?;
        }

        // We expect 2 additional `WillFail` jobs due to retries.
        jobs.push(TestJob::WillFail("expectedfailure".into()));
        jobs.push(TestJob::WillFail("expectedfailure".into()));

        assert_context(context, &jobs).await;
        Ok(())
    }

    #[tokio::test]
    async fn test_worker_queue_timeouts() -> Result<()> {
        let context: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(vec![]));

        let queue = WorkerQueueBuilder::<TestProcessor>::new()
            .with_worker_count(1)
            .with_timeout(Duration::from_secs(1))
            .with_retries(2)
            .with_context(context.clone())
            .build()?;

        let mut jobs = vec![TestJob::Sleep(2)];

        for job in &jobs {
            queue.submit(job.to_owned())?;
        }

        // We expect `Sleep` to be retried as it will take longer
        // than the timeout to complete.
        jobs.push(TestJob::Sleep(2));

        assert_context(context, &jobs).await;
        Ok(())
    }
}
