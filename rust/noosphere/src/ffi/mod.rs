mod error;
mod fs;
mod headers;
mod key;
mod noosphere;
mod sphere;

pub use crate::ffi::noosphere::*;
pub use error::*;
pub use fs::*;
pub use headers::*;
pub use key::*;
pub use sphere::*;

///! This module contains FFI implementation for all C ABI-speaking language
///! integrations.

#[cfg(feature = "headers")]
pub fn generate_headers() -> std::io::Result<()> {
    safer_ffi::headers::builder()
        .to_file("noosphere.h")?
        .generate()
}
