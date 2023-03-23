use std::sync::Arc;

use anyhow::{anyhow, Result};
use safer_ffi::prelude::*;
use tokio::runtime::Runtime as TokioRuntime;
use url::Url;

use crate::{
    ffi::{NsError, TryOrInitialize},
    noosphere::{NoosphereContext, NoosphereContextConfiguration},
    NoosphereNetwork, NoosphereSecurity, NoosphereStorage,
};

#[derive_ReprC(rename = "ns_noosphere")]
#[repr(opaque)]
/// @class ns_noosphere_t
///
/// Core noosphere class.
pub struct NsNoosphere {
    inner: NoosphereContext,
    async_runtime: Arc<TokioRuntime>,
}

impl NsNoosphere {
    pub fn new(
        global_storage_path: &str,
        sphere_storage_path: &str,
        gateway_api: Option<&Url>,
    ) -> Result<Self> {
        Ok(NsNoosphere {
            inner: NoosphereContext::new(NoosphereContextConfiguration {
                storage: NoosphereStorage::Scoped {
                    path: sphere_storage_path.into(),
                },
                security: NoosphereSecurity::Insecure {
                    path: global_storage_path.into(),
                },
                network: NoosphereNetwork::Http {
                    gateway_api: gateway_api.cloned(),
                    ipfs_gateway_url: None,
                },
            })?,
            async_runtime: Arc::new(TokioRuntime::new()?),
        })
    }

    pub fn async_runtime(&self) -> Arc<TokioRuntime> {
        self.async_runtime.clone()
    }

    pub fn inner(&self) -> &NoosphereContext {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut NoosphereContext {
        &mut self.inner
    }
}

#[ffi_export]
/// Initialize a [NoosphereContext] and return a boxed pointer to it. This is
/// the entrypoint to the Noosphere API, and the returned pointer is used to
/// invoke almost all other API functions.
///
/// In order to initialize the [NoosphereContext], you must provide two
/// namespace strings: one for "global" Noosphere configuration, and another for
/// sphere storage. Note that at this time "global" configuration is only used
/// for insecure, on-disk key storage and we will probably deprecate such
/// configuration at a future date.
///
/// You can also initialize the [NoosphereContext] with an optional third
/// argument: a URL string that refers to a Noosphere Gateway API somewhere on
/// the network that one or more local spheres may have access to.
///
/// For this and all other ns_* functions that return a pointer, the pointer
/// should be considered to refer to owned data unless specified otherwise in
/// documentation. This means that you _must_ manually free all data given to
/// you by the FFI (using the appropriate ns-prefixed "free" function).
pub fn ns_initialize(
    global_storage_path: char_p::Ref<'_>,
    sphere_storage_path: char_p::Ref<'_>,
    gateway_url: Option<char_p::Ref<'_>>,
    error_out: Option<Out<'_, repr_c::Box<NsError>>>,
) -> Option<repr_c::Box<NsNoosphere>> {
    error_out.try_or_initialize(|| {
        let gateway_url = match gateway_url {
            Some(raw_url) => Some(Url::parse(raw_url.to_str()).map_err(|error| anyhow!(error))?),
            None => None,
        };

        Ok(Box::new(NsNoosphere::new(
            global_storage_path.to_str(),
            sphere_storage_path.to_str(),
            gateway_url.as_ref(),
        )?)
        .into())
    })
}

#[ffi_export]
/// De-allocate a [NoosphereContext]. Note that this will also drop every
/// [SphereContext] that remains active within the [NoosphereContext].
pub fn ns_free(noosphere: repr_c::Box<NsNoosphere>) {
    drop(noosphere)
}

#[ffi_export]
/// De-allocate a Noosphere-allocated byte array
pub fn ns_bytes_free(bytes: c_slice::Box<u8>) {
    drop(bytes)
}

#[ffi_export]
/// De-allocate a Noosphere-allocated string
pub fn ns_string_free(string: char_p::Box) {
    drop(string)
}

#[ffi_export]
/// De-allocate a Noosphere-allocated array of strings
pub fn ns_string_array_free(string_array: c_slice::Box<char_p::Box>) {
    drop(string_array)
}
