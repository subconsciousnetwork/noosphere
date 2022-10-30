use std::collections::BTreeMap;

use lazy_static::lazy_static;
use safer_ffi::prelude::*;

use crate::{
    platform::{PlatformKeyMaterial, PlatformStore},
    sphere::SphereContext,
};

lazy_static! {
    static ref ACTIVE_SPHERES: BTreeMap<SphereHandle, Box<SphereContext<'static, PlatformKeyMaterial, PlatformStore>>> =
        BTreeMap::new();
}

static mut GLOBAL_STORAGE_PATH: Option<String> = None;
static mut SPHERE_STORAGE_PATH: Option<String> = None;

ReprC! {
    #[repr(C)]
    pub struct SphereHandle {
        user_identity: char_p::Box,
        sphere_identity: char_p::Box,
    }

}

ReprC! {
    #[repr(C)]
    pub struct SphereCreationResult {
        mnemonic: char_p::Box
    }
}

/// Set the global Noosphere storage path. This will be used to store user
/// data and configuration that is cross-cutting with respect to sphere data
#[ffi_export]
pub fn noosphere_set_global_storage_path(path: char_p::Ref<'_>) {
    unsafe {
        GLOBAL_STORAGE_PATH = Some(path.to_string());
    }
}

/// Set the sphere storage path. This will be used as the root for storing
/// sphere data. Spheres will be stored in distinctive sub-hierarchies within
/// this path.
#[ffi_export]
pub fn noosphere_set_sphere_storage_path(path: char_p::Ref<'_>) {
    unsafe {
        SPHERE_STORAGE_PATH = Some(path.to_string());
    }
}

#[cfg(feature = "headers")]
pub fn generate_headers() -> std::io::Result<()> {
    safer_ffi::headers::builder()
        .to_file("noosphere.h")?
        .generate()
}
