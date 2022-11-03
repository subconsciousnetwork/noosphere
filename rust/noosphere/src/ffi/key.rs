use safer_ffi::prelude::*;

use crate::ffi::NoosphereContext;

#[ffi_export]
/// Create a key with the given name in the current platform's support key
/// storage mechanism.
pub fn noosphere_create_key(noosphere: &NoosphereContext, name: char_p::Ref<'_>) {
    pollster::block_on(noosphere.inner().create_key(name.to_str())).unwrap();
}
