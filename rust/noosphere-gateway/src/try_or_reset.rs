use anyhow::Result;
use std::future::Future;
use std::sync::Arc;
use tokio::sync::OnceCell;

/// Wraps an "initialization" step, expressed as a closure, and allows
/// a user to invoke closures with the result of that initialization step
/// as a context argument. The result of invocation is always returned, but
/// a failure result causes the initialized context to be reset so that it
/// is re-initialized upon the next invocation attempt.
///
/// Here is a high-level diagram of the state progression of an invocation:
///
/// ```ignore
/// invoke(Callback)
///        │
///        │context initialized?
///        │
///        ├───no───┐
///        │        ▼
///       yes  Initializer()──Err────┐
///        │        │                │
///        ├────────┘                │
///        │                         │
///        │                         ▼
///        ▼                 ┌───────────────┐
/// Callback(context)──Err──►│ reset context │
///                          └───────────────┘
/// ```
pub struct TryOrReset<I, O, F>
where
    F: Future<Output = Result<O, anyhow::Error>>,
    I: Fn() -> F,
{
    init: I,
    initialized: OnceCell<Arc<O>>,
}

impl<I, O, F> TryOrReset<I, O, F>
where
    F: Future<Output = Result<O, anyhow::Error>>,
    I: Fn() -> F,
{
    pub fn new(init: I) -> Self {
        TryOrReset {
            init,
            initialized: OnceCell::new(),
        }
    }

    /// Invoke a closure with the initialized context. The result will be
    /// returned as normal, but an error result will cause the initialized
    /// context to be reset so that the next time an invocation is attempted,
    /// context initialization will be retried.
    pub async fn invoke<Ii, Oo, Ff>(&mut self, invoke: Ii) -> Result<Oo>
    where
        Ii: FnOnce(Arc<O>) -> Ff,
        Ff: Future<Output = Result<Oo, anyhow::Error>>,
    {
        match self
            .initialized
            .get_or_try_init(|| async { Ok(Arc::new((self.init)().await?)) })
            .await
        {
            Ok(initialized) => match invoke(initialized.clone()).await {
                Ok(output) => Ok(output),
                Err(error) => {
                    self.initialized.take();
                    Err(error)
                }
            },
            Err(error) => Err(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use anyhow::{anyhow, Result};
    use std::{ops::AddAssign, sync::Arc};

    use tokio::sync::Mutex;

    use super::TryOrReset;

    #[tokio::test]
    async fn it_initializes_context_before_invocation_and_recovers_from_failure() {
        let count = Arc::new(Mutex::new(0u32));

        let mut again = TryOrReset::new(|| async {
            let mut count = count.lock().await;
            count.add_assign(1);
            Ok(format!("Hello {}", count))
        });

        again
            .invoke(|context| async move {
                assert_eq!("Hello 1", context.as_str());
                Ok(())
            })
            .await
            .unwrap();

        let _: Result<()> = again
            .invoke(|_| async move { Err(anyhow!("Arbitrary error")) })
            .await;

        again
            .invoke(|context| async move {
                assert_eq!("Hello 2", context.as_str());
                Ok(())
            })
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn it_only_initializes_context_once_as_long_as_results_are_ok() {
        let count = Arc::new(Mutex::new(0u32));

        let mut again = TryOrReset::new(|| async {
            let mut count = count.lock().await;
            count.add_assign(1);
            Ok(format!("Hello {}", count))
        });

        for _ in 0..10 {
            again
                .invoke(|context| async move {
                    assert_eq!("Hello 1", context.as_str());
                    Ok(())
                })
                .await
                .unwrap();
        }
    }

    #[tokio::test]
    async fn it_will_try_again_next_time_if_initialization_fails() {
        let count = Arc::new(Mutex::new(0u32));
        let mut again = TryOrReset::new(|| async {
            let mut count = count.lock().await;
            count.add_assign(1);
            if count.to_owned() == 1 {
                Err(anyhow!("Arbitrary failure"))
            } else {
                Ok(format!("Hello {}", count))
            }
        });

        let _ = again
            .invoke(|_| async move {
                assert!(false, "First initialization should not have succeeded");
                Ok(())
            })
            .await;

        again
            .invoke(|context| async move {
                assert_eq!("Hello 2", context.as_str());
                Ok(())
            })
            .await
            .unwrap()
    }
}
