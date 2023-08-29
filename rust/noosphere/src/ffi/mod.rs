// TODO(getditto/safer_ffi#181): Re-enable this lint
#![allow(clippy::incorrect_clone_impl_on_copy_type)]

mod authority;
mod context;
mod error;
mod headers;
mod key;
mod noosphere;
mod petname;
mod sphere;
mod tracing;

pub use crate::ffi::noosphere::*;
pub use crate::ffi::tracing::*;
pub use authority::*;
pub use context::*;
pub use error::*;
pub use headers::*;
pub use key::*;
pub use petname::*;
pub use sphere::*;

///! This module contains FFI implementation for all C ABI-speaking language
///! integrations.

#[cfg(feature = "headers")]
pub fn generate_headers() -> std::io::Result<()> {
    safer_ffi::headers::builder()
        .to_file("noosphere.h")?
        .generate()
}
