use crate::{ops::OpenStorage, storage::Storage};
use anyhow::Result;
use std::ops::{Deref, DerefMut};

#[cfg(doc)]
use crate::MemoryStorage;

/// An ephemeral [Storage] that does not persist after dropping.
/// Currently, native builds create a temp dir syncing lifetimes, and web
/// builds use a randomly generated database name.
/// In the future, we may have web builds that use
/// a file-system backed Storage, or native builds that do not use
/// the file-system (currently the case with [MemoryStorage]), where
/// a more complex configuration is needed. Mostly used in tests.
pub struct TempStorage<S>
where
    S: Storage + OpenStorage,
{
    inner: S,
    #[cfg(not(target_arch = "wasm32"))]
    _temp_dir: tempfile::TempDir,
}

impl<S> TempStorage<S>
where
    S: Storage + OpenStorage,
{
    /// Create a new [TempStorage], wrapping a new [Storage]
    /// that will be cleared after dropping.
    pub async fn new() -> Result<Self> {
        #[cfg(target_arch = "wasm32")]
        let key: String = witty_phrase_generator::WPGen::new()
            .with_words(3)
            .unwrap()
            .into_iter()
            .map(|word| String::from(word))
            .collect();
        #[cfg(target_arch = "wasm32")]
        let inner = S::open(&key).await?;
        #[cfg(target_arch = "wasm32")]
        let out = Self { inner };

        #[cfg(not(target_arch = "wasm32"))]
        let _temp_dir = tempfile::TempDir::new()?;
        #[cfg(not(target_arch = "wasm32"))]
        let inner = S::open(_temp_dir.path()).await?;
        #[cfg(not(target_arch = "wasm32"))]
        let out = Self { _temp_dir, inner };

        Ok(out)
    }
}

impl<S> Deref for TempStorage<S>
where
    S: Storage + OpenStorage,
{
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<S> DerefMut for TempStorage<S>
where
    S: Storage + OpenStorage,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<S> AsRef<S> for TempStorage<S>
where
    S: Storage + OpenStorage,
{
    fn as_ref(&self) -> &S {
        &self.inner
    }
}
