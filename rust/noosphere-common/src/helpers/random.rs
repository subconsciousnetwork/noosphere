use rand::{rngs::StdRng, Rng, SeedableRng};
use std::sync::Arc;
use tokio::sync::Mutex;

/// A helper to support consistent use of seeded randomness in tests.
/// Its primary purpose is to retain and report the seed in use for a
/// random number generator that can be shared across threads in tests.
/// This is probably not suitable for use outside of tests.
///
/// ```rust
/// # #![cfg(feature = "helpers")]
/// # use anyhow::Result;
/// # use rand::Rng;
/// # use noosphere_common::helpers::TestEntropy;
/// #
/// # #[tokio::main(flavor = "multi_thread")]
/// # async fn main() -> Result<()> {
/// #
/// let test_entropy = TestEntropy::default();
/// let random_int = test_entropy.to_rng().lock().await.gen::<u8>();
///
/// let seeded_entropy = TestEntropy::from_seed(test_entropy.seed().clone());
/// assert_eq!(random_int, seeded_entropy.to_rng().lock().await.gen::<u8>());
/// #
/// #   Ok(())
/// # }
/// ```
pub struct TestEntropy {
    seed: [u8; 32],
    rng: Arc<Mutex<StdRng>>,
}

impl Default for TestEntropy {
    fn default() -> Self {
        Self::from_seed(rand::thread_rng().gen::<[u8; 32]>())
    }
}

impl TestEntropy {
    /// Initialize the [TestEntropy] with an explicit seed
    pub fn from_seed(seed: [u8; 32]) -> Self {
        tracing::info!(?seed, "Initializing test entropy...");

        let rng = Arc::new(Mutex::new(SeedableRng::from_seed(seed)));
        Self { seed, rng }
    }

    /// Get an owned instance of the internal [Rng] initialized by this
    /// [TestEntropy]
    pub fn to_rng(&self) -> Arc<Mutex<StdRng>> {
        self.rng.clone()
    }

    /// Get the seed used to initialize the internal [Rng] of this [TestEntropy]
    pub fn seed(&self) -> &[u8; 32] {
        &self.seed
    }
}
