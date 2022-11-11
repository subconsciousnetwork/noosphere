use std::sync::Arc;

use anyhow::Result;
use safer_ffi::prelude::*;
use tokio::runtime::Runtime as TokioRuntime;
use url::Url;

use crate::noosphere::{NoosphereContext, NoosphereContextConfiguration};

ReprC! {
    #[ReprC::opaque]
    pub struct NsNoosphereContext {
        inner: NoosphereContext,
        async_runtime: Arc<TokioRuntime>
    }
}

impl NsNoosphereContext {
    pub fn new(
        global_storage_path: &str,
        sphere_storage_path: &str,
        gateway_url: Option<&Url>,
    ) -> Result<Self> {
        Ok(NsNoosphereContext {
            inner: NoosphereContext::new(NoosphereContextConfiguration::Insecure {
                global_storage_path: global_storage_path.into(),
                sphere_storage_path: sphere_storage_path.into(),
                gateway_url: gateway_url.cloned(),
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
/// namespace strings: one for "global" Noosphere configuration, and another
/// for sphere storage. Note that at this time "global" configuration is only
/// used for insecure, on-disk key storage and we will probably deprecate such
/// configuration at a future date.
///
/// You can also initialize the [NoosphereContext] with an optional third
/// argument: a URL string that refers to a Noosphere Gateway API somewhere
/// on the network that one or more local spheres may have access to.
pub fn ns_initialize(
    global_storage_path: char_p::Ref<'_>,
    sphere_storage_path: char_p::Ref<'_>,
    gateway_url: Option<char_p::Ref<'_>>,
) -> repr_c::Box<NsNoosphereContext> {
    repr_c::Box::new(
        NsNoosphereContext::new(
            global_storage_path.to_str(),
            sphere_storage_path.to_str(),
            gateway_url
                .map(|value| Url::parse(value.to_str()).unwrap())
                .as_ref(),
        )
        .unwrap(),
    )
}

#[ffi_export]
/// De-allocate a [NoosphereContext]. Note that this will also drop every
/// [SphereContext] that remains active within the [NoosphereContext].
pub fn ns_free(noosphere: repr_c::Box<NsNoosphereContext>) {
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
