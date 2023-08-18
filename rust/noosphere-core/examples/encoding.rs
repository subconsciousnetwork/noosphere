//! WIP on cross-platform benchmarking our encoder.
//!
//! wasm32 builds use `wasm_bindgen_test` runs as if
//! it were running tests, hence the `wasm_bindgen_test`
//! attribute on functions. Native builds run as expected.
use async_stream::try_stream;
use bytes::Bytes;
use cid::Cid;
use noosphere_core::data::{BodyChunkIpld, BufferStrategy};
use noosphere_core::tracing::initialize_tracing;
use noosphere_storage::{helpers::make_disposable_storage, SphereDb, Storage};
use std::collections::HashMap;
use tokio::{self, io::AsyncRead};
use tokio_stream::{Stream, StreamExt};
use tokio_util::io::StreamReader;

#[cfg(target_arch = "wasm32")]
use web_time::Instant;

#[cfg(not(target_arch = "wasm32"))]
use std::time::Instant;

#[derive(PartialEq, Debug)]
enum BenchmarkPosition {
    Start,
    End,
}

/// Simple timer util to record duration of processing.
/// Does not support nested, overlapping, or duplicate time ranges.
struct EncodingBenchmark {
    name: String,
    timestamps: Vec<(BenchmarkPosition, String, Instant)>,
}

impl EncodingBenchmark {
    pub fn new(name: &str) -> Self {
        EncodingBenchmark {
            name: name.to_owned(),
            timestamps: vec![],
        }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn start(&mut self, name: &str) {
        self.timestamps
            .push((BenchmarkPosition::Start, name.to_owned(), Instant::now()))
    }

    pub fn end(&mut self, name: &str) {
        self.timestamps
            .push((BenchmarkPosition::End, name.to_owned(), Instant::now()))
    }

    pub fn results(&self) -> anyhow::Result<HashMap<String, String>> {
        let mut current: Option<&(BenchmarkPosition, String, Instant)> = None;
        let mut results = HashMap::default();
        for timestamp in self.timestamps.iter() {
            if let Some(current_timestamp) = current {
                assert!(timestamp.0 == BenchmarkPosition::End);
                assert_eq!(timestamp.1, current_timestamp.1);
                let duration = current_timestamp.2.elapsed().as_millis();
                if results
                    .insert(timestamp.1.to_owned(), format!("{}ms", duration))
                    .is_some()
                {
                    return Err(anyhow::anyhow!("Duplicate entry for {}", timestamp.1));
                }
                current = None;
            } else {
                assert!(timestamp.0 == BenchmarkPosition::Start);
                current = Some(timestamp);
            }
        }
        Ok(results)
    }
}

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_test::wasm_bindgen_test;
#[cfg(target_arch = "wasm32")]
wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main]
pub async fn main() -> anyhow::Result<()> {
    initialize_tracing(None);
    bench_100_x_1kb().await?;
    bench_500_x_2kb().await?;
    bench_4_x_256kb().await?;
    bench_10_x_1mb().await?;
    bench_10000_x_1kb().await?;
    Ok(())
}

#[cfg(target_arch = "wasm32")]
pub fn main() {
    initialize_tracing(None);
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
async fn bench_100_x_1kb() -> anyhow::Result<()> {
    run_bench("100 x 1kb", 1024, 100, 0).await
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
async fn bench_500_x_2kb() -> anyhow::Result<()> {
    run_bench("500 x 2kb", 1024 * 2, 500, 0).await
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
async fn bench_4_x_256kb() -> anyhow::Result<()> {
    run_bench("4 x 256kb", 1024 * 256, 4, 0).await
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
async fn bench_10_x_1mb() -> anyhow::Result<()> {
    run_bench("10 x 1mb", 1024 * 1024, 10, 0).await
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
async fn bench_10000_x_1kb() -> anyhow::Result<()> {
    run_bench("10^3 x 1kb", 1024, 1024 * 10, 0).await
}

async fn run_bench(
    name: &str,
    chunk_size: u32,
    chunk_count: usize,
    memory_limit: u64,
) -> anyhow::Result<()> {
    let mut bench = EncodingBenchmark::new(name);
    let provider = make_disposable_storage().await?;
    let db = SphereDb::new(&provider).await?;
    let total_size = chunk_size * <usize as TryInto<u32>>::try_into(chunk_count).unwrap();
    assert!(total_size as u64 > memory_limit);

    let stream = make_stream(chunk_size, chunk_count);
    let reader = StreamReader::new(stream);
    bench.start("encode");
    let cid = encode_stream(reader, &db, memory_limit).await?;
    bench.end("encode");
    bench.start("decode");
    let bytes_read = decode_stream(&cid, &db).await?;
    bench.end("decode");

    assert_eq!(bytes_read, total_size);

    tracing::info!("{}: {:#?}", bench.name(), bench.results());
    Ok(())
}

fn make_stream<'a>(
    chunk_size: u32,
    chunk_count: usize,
) -> impl Stream<Item = Result<Bytes, std::io::Error>> + Unpin + 'a {
    Box::pin(try_stream! {
        for n in 1..=chunk_count {
            let chunk: Vec<u8> = vec![n as u8; <u32 as TryInto<usize>>::try_into(chunk_size).unwrap()];
            yield Bytes::from(chunk);
        }
    })
}

async fn encode_stream<S, R>(content: R, db: &SphereDb<S>, memory_limit: u64) -> anyhow::Result<Cid>
where
    R: AsyncRead + Unpin,
    S: Storage,
{
    BodyChunkIpld::encode(content, db, Some(BufferStrategy::Limit(memory_limit))).await
}

async fn decode_stream<S>(cid: &Cid, db: &SphereDb<S>) -> anyhow::Result<u32>
where
    S: Storage,
{
    let stream = BodyChunkIpld::decode(cid, db);
    tokio::pin!(stream);
    let mut bytes_read: u32 = 0;
    while let Some(chunk) = stream.try_next().await? {
        bytes_read += chunk.len() as u32;
    }
    Ok(bytes_read)
}
