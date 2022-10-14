use std::sync::Once;
use tracing_subscriber::prelude::*;

static INITIALIZE_TRACING: Once = Once::new();

pub fn initialize_tracing() {
    INITIALIZE_TRACING.call_once(|| {
        tracing_subscriber::registry()
            .with(tracing_subscriber::EnvFilter::new(
                std::env::var("RUST_LOG")
                    .unwrap_or_else(|_| "noosphere_cli,orb,tower_http=debug".into()),
            ))
            .with(tracing_subscriber::fmt::layer())
            .init();
    });
}
