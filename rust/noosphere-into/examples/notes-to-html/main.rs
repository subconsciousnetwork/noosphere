#[cfg(not(target_arch = "wasm32"))]
mod implementation;

#[cfg(not(target_arch = "wasm32"))]
#[tokio::main(flavor = "multi_thread")]
pub async fn main() -> anyhow::Result<()> {
    implementation::main().await
}

#[cfg(target_arch = "wasm32")]
pub fn main() {
    println!("Demo not implemented for wasm32");
}
