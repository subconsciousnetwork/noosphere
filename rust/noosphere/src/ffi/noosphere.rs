use anyhow::Result;
use safer_ffi::prelude::*;
use url::Url;

use crate::noosphere::{NoosphereContext as NoosphereContextImpl, NoosphereContextConfiguration};

ReprC! {
    #[ReprC::opaque]
    pub struct NoosphereContext {
        inner: NoosphereContextImpl
    }
}

impl NoosphereContext {
    pub fn new(
        global_storage_path: &str,
        sphere_storage_path: &str,
        gateway_url: Option<&Url>,
    ) -> Result<Self> {
        Ok(NoosphereContext {
            inner: NoosphereContextImpl::new(NoosphereContextConfiguration::Insecure {
                global_storage_path: global_storage_path.into(),
                sphere_storage_path: sphere_storage_path.into(),
                gateway_url: gateway_url.cloned(),
            })?,
        })
    }

    pub fn inner(&self) -> &NoosphereContextImpl {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut NoosphereContextImpl {
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
pub fn noosphere_initialize(
    global_storage_path: char_p::Ref<'_>,
    sphere_storage_path: char_p::Ref<'_>,
    gateway_url: Option<char_p::Ref<'_>>,
) -> repr_c::Box<NoosphereContext> {
    repr_c::Box::new(
        NoosphereContext::new(
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
pub fn noosphere_free(noosphere: repr_c::Box<NoosphereContext>) {
    drop(noosphere)
}
