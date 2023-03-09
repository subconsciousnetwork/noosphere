#![allow(unused_variables)]

use std::str::FromStr;

use anyhow::anyhow;
use cid::Cid;
use noosphere_sphere::{SpherePetnameRead, SpherePetnameWrite, SphereWalker};
use safer_ffi::{char_p::InvalidNulTerminator, prelude::*};

use crate::ffi::{NsError, TryOrInitialize};

use super::{NsNoosphere, NsSphere};

#[ffi_export]
/// Get the sphere identity - a DID - assigned to a given petname of a sphere.
/// Note that this call will produce an error if no address has been assigned to
/// the name.
pub fn ns_sphere_petname_get(
    noosphere: &NsNoosphere,
    sphere: &NsSphere,
    petname: char_p::Ref<'_>,
    error_out: Option<Out<'_, repr_c::Box<NsError>>>,
) -> Option<char_p::Box> {
    error_out.try_or_initialize(|| {
        noosphere.async_runtime().block_on(async {
            sphere
                .inner()
                .get_petname(petname.to_str())
                .await?
                .ok_or_else(|| anyhow!("No petname '{}' has been set", petname.to_str()))?
                .to_string()
                .try_into()
                .map_err(|error: InvalidNulTerminator<String>| anyhow!(error).into())
        })
    })
}

#[ffi_export]
/// Assign a petname to a sphere identity (a DID). This will overwrite the
/// petname with the new sphere identity if it has been set previously. When a
/// new address is assigned to a petname, the petname will be reset to an
/// unresolved state. Make sure to invoke sync after setting new petnames, so
/// that new sphere identities can be resolved by the gateway (if one is
/// configured).
pub fn ns_sphere_petname_set(
    noosphere: &NsNoosphere,
    sphere: &mut NsSphere,
    petname: char_p::Ref<'_>,
    did: Option<char_p::Ref<'_>>,
    error_out: Option<Out<'_, repr_c::Box<NsError>>>,
) {
    error_out.try_or_initialize(|| {
        noosphere.async_runtime().block_on(async {
            sphere
                .inner_mut()
                .set_petname(petname.to_str(), did.map(|did| did.to_str().into()))
                .await?;

            Ok(())
        })
    });
}

#[ffi_export]
/// Resolve a configured petname, using the sphere idenitty - a DID - to
/// determine a link - a CID - that is associated with it. The returned link is
/// a UTF-8, base64-encoded CIDv1 string that may be used to resolve data from
/// the IPFS content space. Note that this call will produce an error if no
/// address has been assigned to the given petname.
pub fn ns_sphere_petname_resolve(
    noosphere: &NsNoosphere,
    sphere: &NsSphere,
    petname: char_p::Ref<'_>,
    error_out: Option<Out<'_, repr_c::Box<NsError>>>,
) -> Option<char_p::Box> {
    error_out.try_or_initialize(|| {
        noosphere.async_runtime().block_on(async {
            sphere
                .inner()
                .resolve_petname(petname.to_str())
                .await?
                .ok_or_else(|| anyhow!("No record resolved for petname '{}'", petname.to_str()))?
                .to_string()
                .try_into()
                .map_err(|error: InvalidNulTerminator<String>| anyhow!(error).into())
        })
    })
}

#[ffi_export]
/// Get an array of all of the petnames in a sphere at the current version.
pub fn ns_sphere_petname_list(
    noosphere: &NsNoosphere,
    sphere: &NsSphere,
    error_out: Option<Out<'_, repr_c::Box<NsError>>>,
) -> c_slice::Box<char_p::Box> {
    let possible_output = error_out.try_or_initialize(|| {
        noosphere.async_runtime().block_on(async {
            let petname_set = SphereWalker::from(sphere.inner()).list_petnames().await?;
            let mut all_petnames: Vec<char_p::Box> = Vec::new();

            for petname in petname_set.into_iter() {
                all_petnames.push(
                    petname
                        .try_into()
                        .map_err(|error: InvalidNulTerminator<String>| anyhow!(error))?,
                );
            }

            Ok(all_petnames)
        })
    });

    match possible_output {
        Some(slugs) => slugs,
        None => Vec::new(),
    }
    .into_boxed_slice()
    .into()
}

#[ffi_export]
/// Get an array of all of the petnames that changed in a given sphere since a
/// given revision of that sphere (excluding the given revision). The revision
/// should be provided as a UTF-8 base64-encoded CIDv1 string. If no revision is
/// provided, the entire history will be considered (back to and including the
/// first revision).
///
/// Note that a petname change may mean the petname was added, updated or
/// removed. Also note that multiple changes to the same petname will be reduced
/// to a single entry in the array that is returned.
///
/// A petname will also be considered changed if it goes from an unresolved
/// state to a resolved state.
pub fn ns_sphere_petname_changes(
    noosphere: &NsNoosphere,
    sphere: &NsSphere,
    since_cid: Option<char_p::Ref<'_>>,
    error_out: Option<Out<'_, repr_c::Box<NsError>>>,
) -> c_slice::Box<char_p::Box> {
    let possible_output = error_out.try_or_initialize(|| {
        noosphere.async_runtime().block_on(async {
            let since = match since_cid {
                Some(cid_string) => {
                    Some(Cid::from_str(cid_string.to_str()).map_err(|error| anyhow!(error))?)
                }
                None => None,
            };

            let changed_petname_set = SphereWalker::from(sphere.inner())
                .petname_changes(since.as_ref())
                .await?;
            let mut changed_petnames: Vec<char_p::Box> = Vec::new();

            for petname in changed_petname_set.into_iter() {
                changed_petnames.push(
                    petname
                        .try_into()
                        .map_err(|error: InvalidNulTerminator<String>| anyhow!(error))?,
                );
            }

            Ok(changed_petnames)
        })
    });

    match possible_output {
        Some(petnames) => petnames,
        None => Vec::new(),
    }
    .into_boxed_slice()
    .into()
}
