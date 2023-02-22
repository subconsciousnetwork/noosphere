mod error;
mod fs;
mod headers;
mod key;
mod noosphere;
mod petname;
mod sphere;

#[cfg(feature = "headers")]
mod header_transformer;

pub use crate::ffi::noosphere::*;
pub use error::*;
pub use fs::*;
pub use headers::*;
pub use key::*;
pub use petname::*;
pub use sphere::*;

///! This module contains FFI implementation for all C ABI-speaking language
///! integrations.

#[cfg(feature = "headers")]
pub fn generate_headers() -> std::io::Result<()> {
    use header_transformer::HeaderTransformer;
    use std::{fs::File, io::BufWriter};
    safer_ffi::headers::builder()
        .to_writer(HeaderTransformer::new(BufWriter::new(File::create(
            "noosphere.h",
        )?))?)
        .generate()
}
