use std::{cell::OnceCell, time::Duration};

use crate::{
    ffi::{NsError, TryOrInitialize},
    noosphere::{NoosphereContext, NoosphereContextConfiguration},
    NoosphereNetwork, NoosphereSecurity, NoosphereStorage,
};
use anyhow::{anyhow, Result};
use pkg_version::*;
use safer_ffi::prelude::*;
use tokio::runtime::Runtime as TokioRuntime;
use url::Url;

#[ffi_export]
#[allow(unused)]
/// Noosphere's major version.
const NOOSPHERE_VERSION_MAJOR: u32 = pkg_version_major!();
#[ffi_export]
#[allow(unused)]
/// Noosphere's minor version.
const NOOSPHERE_VERSION_MINOR: u32 = pkg_version_minor!();
#[ffi_export]
#[allow(unused)]
/// Noosphere's patch version.
const NOOSPHERE_VERSION_PATCH: u32 = pkg_version_patch!();

#[derive_ReprC(rename = "ns_noosphere")]
#[repr(opaque)]
/// @class ns_noosphere_t
/// Root noosphere context, entrypoint of the Noosphere API.
pub struct NsNoosphere {
    inner: NoosphereContext,
    // NOTE: We are using a [OnceCell] primarily so that we can safely
    // take the value at "drop" time.
    async_runtime: OnceCell<TokioRuntime>,
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
            async_runtime: OnceCell::from(TokioRuntime::new()?),
        })
    }

    pub fn async_runtime(&self) -> &TokioRuntime {
        // NOTE: Unwrap is safe because we don't allow initializing
        // [NsNoosphere] with an empty [OnceCell]
        self.async_runtime.get().unwrap()
    }

    pub fn inner(&self) -> &NoosphereContext {
        &self.inner
    }

    pub fn inner_mut(&mut self) -> &mut NoosphereContext {
        &mut self.inner
    }
}

impl Drop for NsNoosphere {
    fn drop(&mut self) {
        if let Some(async_runtime) = self.async_runtime.take() {
            async_runtime.shutdown_timeout(Duration::from_secs(60));
        }
    }
}

#[ffi_export]
/// @memberof ns_noosphere_t
/// Initialize a ns_noosphere_t instance.
///
/// In order to initialize the ns_noosphere_t, you must provide two
/// namespace strings: one for "global" Noosphere configuration, and another for
/// sphere storage. Note that at this time "global" configuration is only used
/// for insecure, on-disk key storage and we will probably deprecate such
/// configuration at a future date.
///
/// You can also initialize the ns_noosphere_t with an optional third
/// argument: a URL string that refers to a Noosphere Gateway API somewhere on
/// the network that one or more local spheres may have access to.
///
/// Note that storages (`global_storage_path`, `sphere_storage_path`) can only
/// be opened by a single `ns_noosphere` at a time.
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
/// @memberof ns_noosphere_t
///
/// Deallocate a ns_noosphere_t instance.
///
/// Disposal of the ns_noosphere_t may take up to 60 seconds, as the underlying
/// implementation will give that amount of time for outstanding asynchronous
/// tasks to complete.
///
/// This will also deallocate every ns_sphere_t that remains active within the
/// ns_noosphere_t.
pub fn ns_free(noosphere: repr_c::Box<NsNoosphere>) {
    drop(noosphere)
}

#[ffi_export]
/// Deallocate a Noosphere-allocated byte array.
pub fn ns_bytes_free(bytes: c_slice::Box<u8>) {
    drop(bytes)
}

#[ffi_export]
/// Deallocate a Noosphere-allocated string.
pub fn ns_string_free(string: char_p::Box) {
    drop(string)
}

#[ffi_export]
/// Deallocate a Noosphere-allocated array of strings.
pub fn ns_string_array_free(string_array: c_slice::Box<char_p::Box>) {
    drop(string_array)
}
