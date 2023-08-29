use rand::{rngs::StdRng, Rng, SeedableRng};
use std::sync::Arc;
use tokio::sync::Mutex;

/// A helper to support consistent use of seeded randomness in tests.
/// Its primary purpose is to retain and report the seed in use for a
/// random number generator that can be shared across threads in tests.
/// This is probably not suitable for use outside of tests.
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
    pub fn from_seed(seed: [u8; 32]) -> Self {
        tracing::info!(?seed, "Initializing test entropy...");

        let rng = Arc::new(Mutex::new(SeedableRng::from_seed(seed.clone())));
        Self { seed, rng }
    }

    pub fn to_rng(&self) -> Arc<Mutex<StdRng>> {
        self.rng.clone()
    }

    pub fn seed(&self) -> &[u8] {
        &self.seed
    }
}
