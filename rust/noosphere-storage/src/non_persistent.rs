use crate::{ops::OpenStorage, storage::Storage};
use anyhow::Result;
use std::ops::{Deref, DerefMut};

#[cfg(doc)]
use crate::EphemeralStorage;
#[cfg(doc)]
use crate::MemoryStorage;

/// [Storage] provider wrapper that does not persist after dropping.
///
/// Whereas [EphemeralStorage] can provide a slice of a storage system
/// as non-persistent storage space, the entirety of [NonPersistentStorage]
/// is wiped after dropping.
///
/// Currently, native builds create a temp dir syncing lifetimes, and web
/// builds use a randomly generated database name.
/// In the future, we may have web builds that use
/// a file-system backed Storage, or native builds that do not use
/// the file-system (currently the case with [MemoryStorage]), where
/// a more complex configuration is needed. Mostly used in tests.
pub struct NonPersistentStorage<S>
where
    S: Storage + OpenStorage,
{
    inner: S,
    #[cfg(not(target_arch = "wasm32"))]
    _temp_dir: tempfile::TempDir,
}

impl<S> NonPersistentStorage<S>
where
    S: Storage + OpenStorage,
{
    /// Create a new [NonPersistentStorage], wrapping a new [Storage]
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

impl<S> Deref for NonPersistentStorage<S>
where
    S: Storage + OpenStorage,
{
    type Target = S;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<S> DerefMut for NonPersistentStorage<S>
where
    S: Storage + OpenStorage,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.inner
    }
}

impl<S> AsRef<S> for NonPersistentStorage<S>
where
    S: Storage + OpenStorage,
{
    fn as_ref(&self) -> &S {
        &self.inner
    }
}
