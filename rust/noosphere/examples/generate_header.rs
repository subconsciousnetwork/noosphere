///! This example is used to generate the FFI interface C header. You can run
///! it locally to generate a noosphere.h that represents the FFI interface
///! exposed by this crate at any given revision.
use anyhow::{anyhow, Result};

#[cfg(feature = "headers")]
fn main() -> Result<()> {
    noosphere::ffi::generate_headers().map_err(|e| anyhow!(e.to_string()))?;

    Ok(())
}

#[cfg(not(feature = "headers"))]
fn main() -> Result<()> {
    Err(anyhow!("Must run with \"headers\" feature."))
}
