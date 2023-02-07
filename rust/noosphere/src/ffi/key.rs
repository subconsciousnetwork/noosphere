use safer_ffi::prelude::*;

use crate::ffi::{NsError, NsNoosphereContext, TryOrInitialize};

#[ffi_export]
/// Create a key with the given name in the current platform's support key
/// storage mechanism.
pub fn ns_key_create(
    noosphere: &NsNoosphereContext,
    name: char_p::Ref<'_>,
    error_out: Option<Out<'_, repr_c::Box<NsError>>>,
) {
    error_out.try_or_initialize(|| {
        noosphere
            .async_runtime()
            .block_on(noosphere.inner().create_key(name.to_str()))
            .map_err(|error| error.into())
    });
}
