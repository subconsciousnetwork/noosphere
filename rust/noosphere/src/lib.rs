#[macro_use]
extern crate tracing;

pub mod implementation;

#[cfg(not(target_arch = "wasm32"))]
pub mod bindings;
#[cfg(not(target_arch = "wasm32"))]
pub mod ffi;
#[cfg(target_arch = "wasm32")]
pub mod wasm;

// Uniffi Bindings
// Here we declare custom types/converters (e.g. `Url`), and
// we must import all exported types, as they must be in the
// crate's top-level lib, along with the scaffolding.

#[cfg(not(target_arch = "wasm32"))]
uniffi::include_scaffolding!("noosphere");

/// Exported Uniffi types
#[cfg(not(target_arch = "wasm32"))]
use crate::bindings::NoosphereContext;
#[cfg(not(target_arch = "wasm32"))]
use crate::bindings::SphereReceipt;
#[cfg(not(target_arch = "wasm32"))]
use crate::implementation::NoosphereError;

/// Custom Uniffi types and converters
#[cfg(not(target_arch = "wasm32"))]
use url::Url;
#[cfg(not(target_arch = "wasm32"))]
impl crate::UniffiCustomTypeConverter for Url {
    type Builtin = String;

    fn into_custom(val: Self::Builtin) -> uniffi::Result<Self> {
        Ok(Url::parse(&val)?)
    }

    fn from_custom(obj: Self) -> Self::Builtin {
        obj.into()
    }
}
