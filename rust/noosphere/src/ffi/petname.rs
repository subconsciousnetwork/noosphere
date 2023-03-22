#![allow(unused_variables)]

use std::str::FromStr;

use anyhow::anyhow;
use cid::Cid;
use noosphere_sphere::{SpherePetnameRead, SpherePetnameWrite, SphereWalker};
use safer_ffi::{char_p::InvalidNulTerminator, prelude::*};

use crate::ffi::{NsError, TryOrInitialize};

use super::{NsNoosphere, NsSphere};

#[ffi_export]
/// Returns true if the given petname has been assigned to a sphere identity. If
/// it returns false, it implies one of the following: the petname has never
/// been assigned to any sphere identity, _or_ it was previously assigned to a
/// sphere identity at least once but has since been unassigned.
pub fn ns_sphere_petname_is_set(
    noosphere: &NsNoosphere,
    sphere: &NsSphere,
    petname: char_p::Ref<'_>,
    error_out: Option<Out<'_, repr_c::Box<NsError>>>,
) -> u8 {
    if let Some(result) = error_out.try_or_initialize(|| {
        noosphere.async_runtime().block_on(async {
            Ok(match sphere.inner().get_petname(petname.to_str()).await? {
                Some(_) => true,
                None => false,
            })
        })
    }) {
        if result {
            1
        } else {
            0
        }
    } else {
        0
    }
}

#[ffi_export]
/// Get the sphere identity - a DID - that the given petname is assigned to in
/// the sphere. Note that this call will produce an error if the petname has not
/// been assigned to a sphere identity (or was previously assigned to a sphere
/// identity but has since been unassigned).
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
/// petname so that it is assigned to the new sphere identity if it had been
/// assigned to a different sphere identity previously.
///
/// When a petname is assigned to a new sphere identity, its entry in the
/// address book will be set to an unresolved state. You may pass null as the
/// DID to effective unassign the petname from any sphere identity.
///
/// Make sure to invoke sync after assigning or unassigning petnames to sphere
/// identities, so that newly introduced sphere identities can be resolved by
/// the gateway (if one is configured). Once the gateway's resolutions are
/// sync'd, the related address book entries will be considered to be in a
/// resolved state.
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
/// Resolve a configured petname, using the sphere identity that it is assigned
/// to and determining a link - a CID - that is associated with it. The returned
/// link is a UTF-8, base64-encoded CIDv1 string that may be used to resolve
/// data from the IPFS content space. Note that this call will produce an error
/// if no address has been assigned to the given petname.
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
