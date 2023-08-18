use anyhow::{anyhow, Result};
use async_stream::try_stream;
use async_trait::async_trait;
use noosphere_storage::{KeyValueStore, MemoryStore, Scratch};
use serde::{de::DeserializeOwned, Serialize};
use tokio_stream::{Stream, StreamExt};

/// Takes a [Stream] and returns a stream that yields the items
/// in reverse. A [Scratch] provider is needed when `memory_limit` is
/// reached in order to buffer a large stream.
pub fn reverse_stream<'a, St, S, T, E>(
    stream: St,
    provider: &'a S,
    memory_limit: u64,
) -> impl Stream<Item = Result<T, anyhow::Error>> + Unpin + 'a
where
    T: Reversable + 'static,
    St: Stream<Item = Result<T, E>> + 'a,
    S: Scratch,
    E: Into<anyhow::Error>,
{
    Box::pin(try_stream! {
        tokio::pin!(stream);
        let mut mem_store = Some(ReversableStore::new(MemoryStore::default()));
        let mut db_store: Option<ReversableStore<T, S::ScratchStore>> = None;

        while let Some(item) = stream.try_next().await? {
            if let Some(store) = db_store.as_mut() {
                store.push(item).await?;
            } else if let Some(store) = mem_store.as_mut() {
                store.push(item).await?;

                if store.byte_length() > memory_limit {
                    let previous_store = mem_store.take().unwrap();
                    let scratch_store = provider.get_scratch_store().await?;
                    db_store = Some(ReversableStore::from_store(scratch_store, previous_store).await?);
                }
            }
        }

        if let Some(store) = db_store.take() {
            let output = store.into_reverse_stream().await;
            tokio::pin!(output);
            while let Some(out) = output.try_next().await? {
                yield out;
            }
        } else if let Some(store) = mem_store.take() {
            let output = store.into_reverse_stream().await;
            tokio::pin!(output);
            while let Some(out) = output.try_next().await? {
                yield out;
            }
        } else {
            panic!("Unrecoverable reversable stream state.");
        };
    })
}

/// An accumulating store that can stream out its items
/// forward or in reverse.
struct ReversableStore<T: Reversable, S: ReversableStorage<T>> {
    item_count: usize,
    byte_length: u64,
    store: S,
    _marker: std::marker::PhantomData<T>,
}

impl<T, S> ReversableStore<T, S>
where
    T: Reversable + 'static,
    S: ReversableStorage<T>,
{
    fn new(store: S) -> Self {
        ReversableStore {
            item_count: 0,
            byte_length: 0,
            store,
            _marker: std::marker::PhantomData,
        }
    }

    /// Drains `other` store into a newly created store using `inner`.
    async fn from_store<U: KeyValueStore>(inner: S, other: ReversableStore<T, U>) -> Result<Self> {
        let mut store = ReversableStore::new(inner);
        let mut stream = other.into_forward_stream().await;
        while let Some(item) = stream.try_next().await? {
            store.push(item).await?;
        }
        Ok(store)
    }

    /// Push a new `item` to the store.
    async fn push(&mut self, item: T) -> Result<()> {
        self.byte_length += item.size_of();
        self.store.push(item, self.item_count).await?;
        self.item_count += 1;
        Ok(())
    }

    /// Get total byte length of all items in the store.
    pub fn byte_length(&self) -> u64 {
        self.byte_length
    }

    /// Drain this store's items in a forward stream.
    pub async fn into_forward_stream(
        mut self,
    ) -> impl Stream<Item = Result<T, anyhow::Error>> + Unpin {
        Box::pin(try_stream! {
            for index in 0..self.item_count {
                let item = self.store.get(index).await?;
                yield item;
            }
        })
    }

    /// Drain this store's items in a reverse stream.
    pub async fn into_reverse_stream(
        mut self,
    ) -> impl Stream<Item = Result<T, anyhow::Error>> + Unpin {
        Box::pin(try_stream! {
            for n in 0..self.item_count {
                let index = self.item_count - n - 1;
                let item = self.store.get(index).await?;
                yield item;
            }
        })
    }
}

/// A trait for interacting with [KeyValueStore]s as an
/// immutable stack.
#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
trait ReversableStorage<T> {
    /// Push a new item into the store.
    async fn push(&mut self, item: T, index: usize) -> Result<()>;
    /// Retrieve an item from the store.
    async fn get(&mut self, index: usize) -> Result<T>;
}

#[cfg_attr(not(target_arch = "wasm32"), async_trait)]
#[cfg_attr(target_arch = "wasm32", async_trait(?Send))]
impl<S: KeyValueStore, T: Reversable + 'static> ReversableStorage<T> for S {
    async fn push(&mut self, item: T, index: usize) -> Result<()> {
        let key = format!("{}", index);
        self.set_key(&key, item).await?;
        Ok(())
    }

    async fn get(&mut self, index: usize) -> Result<T> {
        let key = format!("{}", index);
        let item = self.get_key(&key).await?;
        item.ok_or_else(|| anyhow!("Missing chunk."))
    }
}

#[cfg(target_arch = "wasm32")]
pub trait Sendable {}
#[cfg(target_arch = "wasm32")]
impl<T> Sendable for T {}
#[cfg(not(target_arch = "wasm32"))]
pub trait Sendable: Send {}
#[cfg(not(target_arch = "wasm32"))]
impl<T: Send> Sendable for T {}

/// Helper trait for item types supported by `reverse_stream`.
/// Currently only implemented for `Vec<u8>`, other types may be
/// supported in the future.
pub trait Reversable: Sendable + Serialize + DeserializeOwned {
    /// Get the byte size of this item.
    fn size_of(&self) -> u64;
}

impl Reversable for Vec<u8> {
    fn size_of(&self) -> u64 {
        self.len() as u64
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_stream::stream;
    use noosphere_storage::helpers::make_disposable_storage;
    use tokio_stream::StreamExt;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;
    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);
    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_reverses_streams() -> Result<()> {
        let provider = make_disposable_storage().await?;
        let input_stream = Box::pin(stream! {
            for n in 1..=5 {
                yield Result::<_>::Ok(vec![n]);
            }
        });
        let mut reversed = reverse_stream(input_stream, &provider, 1024);
        let mut output = vec![];
        while let Some(value) = reversed.try_next().await? {
            output.push(value);
        }
        assert_eq!(output, vec![vec![5], vec![4], vec![3], vec![2], vec![1],]);
        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_reverses_large_streams() -> Result<()> {
        let chunk_size: u32 = 1024;
        let memory_limit = chunk_size * 5;
        let item_count = 10;
        let provider = make_disposable_storage().await?;
        let input_stream = Box::pin(stream! {
            for n in 1..=item_count {
                let chunk: Vec<u8> = vec![n; chunk_size.try_into().unwrap()];
                yield Result::<_>::Ok(chunk);
            }
        });

        assert!(
            memory_limit < (chunk_size * <u8 as Into<u32>>::into(item_count)),
            "memory limit will be surpassed"
        );

        let mut reversed = reverse_stream(input_stream, &provider, memory_limit.into());
        let mut counter = 10;
        while let Some(value) = reversed.try_next().await? {
            assert_eq!(value, vec![counter; chunk_size.try_into().unwrap()]);
            counter -= 1;
        }
        assert_eq!(counter, 0);
        Ok(())
    }
}
