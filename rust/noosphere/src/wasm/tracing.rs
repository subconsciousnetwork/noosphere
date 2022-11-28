use std::sync::Once;

static INITIALIZE_TRACING: Once = Once::new();

pub fn initialize_tracing() {
    INITIALIZE_TRACING.call_once(|| {
        console_error_panic_hook::set_once();
        tracing_wasm::set_as_global_default();
    })
}
