#[macro_use]
extern crate tracing;

pub mod error;
#[cfg(not(target_arch = "wasm32"))]
pub mod ffi;

#[cfg(target_arch = "wasm32")]
pub mod wasm;

pub mod key;

mod noosphere;
pub use crate::noosphere::*;

pub mod platform;
pub mod sphere;
pub mod storage;

// We need to import types used in the uniffi
use crate::error::NoosphereError;
use crate::ffi::NsNoosphere;
use url::Url;

impl crate::UniffiCustomTypeConverter for Url {
    type Builtin = String;

    fn into_custom(val: Self::Builtin) -> uniffi::Result<Self> {
        Ok(Url::parse(&val)?)
    }

    fn from_custom(obj: Self) -> Self::Builtin {
        obj.into()
    }
}

#[cfg(not(target_arch = "wasm32"))]
uniffi::include_scaffolding!("noosphere");
