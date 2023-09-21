//! Benchmarking suite for comparing [Storage] providers.
//! Even though the `performance` feature is defined in the example manifest,
//! it still needs to be passed in when invoking from the parent crate.
//!
//! Run Sled:
//! `cargo run --example bench`
//! Run RocksDB:
//! `cargo run --example bench --features rocksdb`
//! Run IndexedDb (open `http://localhost:8000` in a browser)
//! `NO_HEADLESS=1 cargo run --example bench --target wasm32-unknown-unknown`

extern crate noosphere_core_dev as noosphere_core;

mod performance;
use anyhow::Result;
use noosphere_common::helpers::TestEntropy;
use noosphere_core::{
    authority::Access,
    context::{HasMutableSphereContext, SphereContentRead, SphereContentWrite},
    data::ContentType,
    helpers::generate_sphere_context,
    tracing::initialize_tracing,
};
use noosphere_storage::{SphereDb, Storage};
use performance::{PerformanceStats, PerformanceStorage};
use rand::Rng;
use tokio::io::AsyncReadExt;

#[cfg(target_arch = "wasm32")]
macro_rules! output {
    ($($v:expr),+) => { tracing::info!($($v,)*) }
}
#[cfg(not(target_arch = "wasm32"))]
macro_rules! output {
    ($($v:expr),+) => { println!($($v,)*) }
}

async fn create_sphere_with_long_history<S: Storage + 'static>(db: SphereDb<S>) -> Result<()> {
    let (mut ctx, _) = generate_sphere_context(Access::ReadWrite, db).await?;
    let entropy = TestEntropy::default();
    let rng_base = entropy.to_rng();
    let mut rng = rng_base.lock().await;

    // Long history, small-ish files
    for _ in 0..100 {
        let random_index = rng.gen_range(0..100);
        let mut random_bytes = Vec::from(rng.gen::<[u8; 32]>());
        let slug = format!("slug{}", random_index);

        let next_bytes = if let Some(mut file) = ctx.read(&slug).await? {
            let mut file_bytes = Vec::new();
            file.contents.read_to_end(&mut file_bytes).await?;
            file_bytes.append(&mut random_bytes);
            file_bytes
        } else {
            random_bytes
        };

        ctx.write(&slug, &ContentType::Bytes, next_bytes.as_ref(), None)
            .await?;
        ctx.save(None).await?;
    }
    Ok(())
}

async fn create_sphere_with_large_files<S: Storage + 'static>(db: SphereDb<S>) -> Result<()> {
    let (mut ctx, _) = generate_sphere_context(Access::ReadWrite, db).await?;
    let entropy = TestEntropy::default();
    let rng_base = entropy.to_rng();
    let mut rng = rng_base.lock().await;

    // Modest history, large-ish files
    for _ in 0..10 {
        let mut random_bytes = (0..1000).fold(Vec::new(), |mut bytes, _| {
            bytes.append(&mut Vec::from(rng.gen::<[u8; 32]>()));
            bytes
        });
        let random_index = rng.gen_range(0..10);
        let slug = format!("slug{}", random_index);

        let next_bytes = if let Some(mut file) = ctx.read(&slug).await? {
            let mut file_bytes = Vec::new();
            file.contents.read_to_end(&mut file_bytes).await?;
            file_bytes.append(&mut random_bytes);
            file_bytes
        } else {
            random_bytes
        };

        ctx.write(&slug, &ContentType::Bytes, next_bytes.as_ref(), None)
            .await?;

        ctx.save(None).await?;
    }
    Ok(())
}

#[cfg(all(
    not(target_arch = "wasm32"),
    not(any(feature = "rocksdb", feature = "sqlite"))
))]
type ActiveStoragePrimitive = noosphere_storage::SledStorage;
#[cfg(all(not(target_arch = "wasm32"), feature = "rocksdb"))]
type ActiveStoragePrimitive = noosphere_storage::RocksDbStorage;
#[cfg(all(
    not(target_arch = "wasm32"),
    feature = "sqlite",
    not(feature = "rocksdb")
))]
type ActiveStoragePrimitive = noosphere_storage::SqliteStorage;
#[cfg(target_arch = "wasm32")]
type ActiveStoragePrimitive = noosphere_storage::IndexedDbStorage;

type ActiveStorageType = PerformanceStorage<ActiveStoragePrimitive>;

struct BenchmarkStorage {
    storage: ActiveStorageType,
    #[cfg(not(target_arch = "wasm32"))]
    _temp_dir: tempfile::TempDir,
    name: String,
}
impl BenchmarkStorage {
    pub async fn new() -> Result<Self> {
        #[cfg(not(target_arch = "wasm32"))]
        let temp_dir = tempfile::TempDir::new()?;
        #[cfg(not(target_arch = "wasm32"))]
        let storage_path = temp_dir.path();

        #[cfg(all(
            not(target_arch = "wasm32"),
            not(any(feature = "rocksdb", feature = "sqlite"))
        ))]
        let (storage, storage_name) = {
            (
                noosphere_storage::SledStorage::new(&storage_path)?,
                "SledDbStorage",
            )
        };

        #[cfg(all(not(target_arch = "wasm32"), feature = "rocksdb"))]
        let (storage, storage_name) = {
            (
                noosphere_storage::RocksDbStorage::new(&storage_path).await?,
                "RocksDbStorage",
            )
        };

        #[cfg(target_arch = "wasm32")]
        let (storage, storage_name) = {
            let temp_name: String = witty_phrase_generator::WPGen::new()
                .with_words(3)
                .unwrap()
                .into_iter()
                .map(|word| String::from(word))
                .collect();
            (
                noosphere_storage::IndexedDbStorage::new(&temp_name).await?,
                "IndexedDbStorage",
            )
        };

        let storage = PerformanceStorage::new(storage);

        #[cfg(not(target_arch = "wasm32"))]
        {
            Ok(BenchmarkStorage {
                storage,
                name: storage_name.to_owned(),
                _temp_dir: temp_dir,
            })
        }
        #[cfg(target_arch = "wasm32")]
        {
            Ok(BenchmarkStorage {
                storage,
                name: storage_name.to_owned(),
            })
        }
    }

    pub async fn sphere_db(&self) -> Result<SphereDb<ActiveStorageType>> {
        SphereDb::new(self.storage.clone()).await
    }

    pub async fn as_stats(&mut self) -> Result<PerformanceStats> {
        self.storage.as_stats().await
    }

    /// Cleanup the storage. Tempdirs handle native implementations,
    /// wipe any IndexedDb usage here.
    pub async fn dispose(self) -> Result<()> {
        #[cfg(target_arch = "wasm32")]
        self.storage.to_inner().clear().await?;
        Ok(())
    }
}

async fn log_perf_stats(stats: &PerformanceStats) {
    output!("reads: {} (avg {}us)", stats.reads.count, stats.reads.mean);
    output!(
        "writes: {} (avg {}us)",
        stats.writes.count,
        stats.writes.mean
    );
    output!(
        "removes: {} (avg {}us)",
        stats.removes.count,
        stats.removes.mean
    );
    output!(
        "flushes: {} (avg {}us)",
        stats.flushes.count,
        stats.flushes.mean
    );
    output!("logical bytes: {}", stats.logical_bytes_stored);
    output!("physical bytes: {}", stats.physical_bytes_stored);

    let space_amplification = if stats.logical_bytes_stored == 0 {
        0.0
    } else {
        stats.physical_bytes_stored as f64 / stats.logical_bytes_stored as f64
    };
    output!("space amplification: {}", space_amplification);
}

async fn bench_sphere_writing_long_history() {
    let mut storage = BenchmarkStorage::new().await.unwrap();
    output!("Testing {}", storage.name);

    let db = storage.sphere_db().await.unwrap();
    create_sphere_with_long_history(db).await.unwrap();
    let stats = storage.as_stats().await.unwrap();
    log_perf_stats(&stats).await;
    storage.dispose().await.unwrap();
}

async fn bench_sphere_writing_large_files() {
    let mut storage = BenchmarkStorage::new().await.unwrap();
    output!("Testing {}", storage.name);

    let db = storage.sphere_db().await.unwrap();
    create_sphere_with_large_files(db).await.unwrap();
    let stats = storage.as_stats().await.unwrap();
    log_perf_stats(&stats).await;
    storage.dispose().await.unwrap();
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test;
#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[cfg(target_arch = "wasm32")]
pub fn main() {}

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen_test]
async fn main_js() {
    initialize_tracing(None);

    bench_sphere_writing_long_history().await;
    bench_sphere_writing_large_files().await;
}

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main(flavor = "multi_thread")]
pub async fn main() {
    initialize_tracing(None);

    bench_sphere_writing_long_history().await;
    bench_sphere_writing_large_files().await;
}
