// TODO(getditto/safer_ffi#181): Re-enable this lint
#![allow(clippy::incorrect_clone_impl_on_copy_type, non_snake_case)]

//! This module defins a C FFI for Noosphere, suitable for cross-language
//! embedding on many different targets

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

#[cfg(feature = "headers")]
pub fn generate_headers() -> std::io::Result<()> {
    safer_ffi::headers::builder()
        .to_file("noosphere.h")?
        .generate()
}
