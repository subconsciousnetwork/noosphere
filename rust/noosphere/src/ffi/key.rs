use safer_ffi::prelude::*;

use crate::ffi::NsNoosphereContext;

#[ffi_export]
/// Create a key with the given name in the current platform's support key
/// storage mechanism.
pub fn ns_key_create(noosphere: &NsNoosphereContext, name: char_p::Ref<'_>) {
    noosphere
        .async_runtime()
        .block_on(noosphere.inner().create_key(name.to_str()))
        .unwrap();
}
