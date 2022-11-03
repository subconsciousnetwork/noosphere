///! This example is used to generate the FFI interface C header. You can run
///! it locally to generate a noosphere.h that represents the FFI interface
///! exposed by this crate at any given revision.

fn main() -> std::io::Result<()> {
    #[cfg(feature = "headers")]
    noosphere::ffi::generate_headers()?;

    Ok(())
}
