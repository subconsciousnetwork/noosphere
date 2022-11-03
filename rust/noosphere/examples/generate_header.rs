fn main() -> std::io::Result<()> {
    #[cfg(feature = "headers")]
    noosphere::ffi::generate_headers()?;

    Ok(())
}
