#![allow(unused_variables)]

use safer_ffi::prelude::*;

use crate::ffi::NsError;

use super::NsNoosphereContext;

#[ffi_export]
/// Get the DID that is assigned to the provided petname; note that the DID is
/// the ID of the sphere, but in order to read the sphere you must resolve the
/// DID to a CID, which tells you the version of the sphere to read.
pub fn ns_sphere_petname_get(
    noosphere: &NsNoosphereContext,
    sphere_identity: char_p::Ref<'_>,
    petname: char_p::Ref<'_>,
    error_out: Option<Out<'_, repr_c::Box<NsError>>>,
) -> Option<char_p::Box> {
    todo!();
}

#[ffi_export]
/// Assign a DID to a petname. This will overwrite a petname entry if one already exists
/// with the given name (and reset the resolved CID, if any).
pub fn ns_sphere_petname_set(
    noosphere: &NsNoosphereContext,
    sphere_identity: char_p::Ref<'_>,
    petname: char_p::Ref<'_>,
    did: char_p::Ref<'_>,
    error_out: Option<Out<'_, repr_c::Box<NsError>>>,
) {
    todo!();
}

#[ffi_export]
/// Resolve a configured petname to a sphere version (a CID), via the DID that
/// has been assigned to it. The returned value is a UTF-8, base64-encoded CIDv1
/// string. If no DID has been assigned to the given petname, no value will be
/// resolved.
pub fn ns_sphere_petname_resolve(
    noosphere: &NsNoosphereContext,
    sphere_identity: char_p::Ref<'_>,
    petname: char_p::Ref<'_>,
    error_out: Option<Out<'_, repr_c::Box<NsError>>>,
) -> Option<char_p::Box> {
    todo!();
}
