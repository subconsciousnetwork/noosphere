///! Helpers to wrangle logging across Noosphere crates
///! NOTE: [initialize_tracing] should only ever be called in tests or binaries;
///! a library should only concern itself with instrumentation and logging.
use std::sync::Once;

static INITIALIZE_TRACING: Once = Once::new();

#[cfg(target_arch = "wasm32")]
pub fn initialize_tracing() {
    INITIALIZE_TRACING.call_once(|| {
        console_error_panic_hook::set_once();
        tracing_wasm::set_as_global_default();
    })
}

#[cfg(not(target_arch = "wasm32"))]
pub fn initialize_tracing() {
    use tracing_subscriber::prelude::*;
    INITIALIZE_TRACING.call_once(|| {
        tracing_subscriber::registry()
            .with(tracing_subscriber::EnvFilter::new(
                std::env::var("RUST_LOG")
                    //.unwrap_or_else(|_| "noosphere_cli,orb,tower_http=debug".into()),
                    .unwrap_or_else(|_| {
                        "noosphere_gateway,noosphere_ipfs=info,noosphere_ns,noosphere_storage"
                            .into()
                    }),
            ))
            .with(tracing_subscriber::fmt::layer())
            .init();
    });
}
