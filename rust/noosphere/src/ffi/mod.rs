mod key;
mod noosphere;
mod sphere;

pub use crate::ffi::noosphere::*;
pub use key::*;
pub use sphere::*;

#[cfg(feature = "headers")]
pub fn generate_headers() -> std::io::Result<()> {
    safer_ffi::headers::builder()
        .to_file("noosphere.h")?
        .generate()
}
