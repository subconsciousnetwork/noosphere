use std::ffi::c_void;

use cid::Cid;
use noosphere_core::{
    authority::Authorization,
    data::{Did, Mnemonic},
};
use noosphere_sphere::{HasMutableSphereContext, SphereAuthorityEscalate, SphereAuthorityWrite};
use safer_ffi::prelude::*;

use crate::{
    error::NoosphereError,
    ffi::{NsError, NsNoosphere, NsSphere},
};

#[ffi_export]
/// @memberof ns_sphere_t
///
/// Authorize another key to manipulate this sphere, given a display name for
/// the authorization and the DID of the key to be authorized.
///
/// The callback arguments are (in order):
///
///  1. The context argument provided in the original call to
///     ns_sphere_authority_authorize
///  2. An owned pointer to an ns_error_t if there was an error, otherwise NULL
///  3. An owned pointer to constant string (the authorization CID) if the call
///     was successful, otherwise NULL
pub fn ns_sphere_authority_authorize(
    noosphere: &NsNoosphere,
    sphere: &mut NsSphere,
    name: char_p::Ref<'_>,
    did: char_p::Ref<'_>,
    context: Option<repr_c::Box<c_void>>,
    callback: extern "C" fn(
        Option<repr_c::Box<c_void>>,
        Option<repr_c::Box<NsError>>,
        Option<char_p::Box>,
    ),
) {
    let mut sphere = sphere.inner_mut().clone();
    let name = name.to_string();
    let did = Did(did.to_string());
    let async_runtime = noosphere.async_runtime();

    noosphere.async_runtime().spawn(async move {
        let result = async {
            let authorization: char_p::Box = sphere
                .authorize(&name, &did)
                .await?
                .to_string()
                .try_into()?;

            Ok(authorization) as Result<_, anyhow::Error>
        }
        .await;

        match result {
            Ok(authorization) => {
                async_runtime.spawn_blocking(move || callback(context, None, Some(authorization)))
            }
            Err(error) => async_runtime.spawn_blocking(move || {
                callback(context, Some(NoosphereError::from(error).into()), None)
            }),
        };
    });
}

#[ffi_export]
/// @memberof ns_sphere_t
///
/// Given a recovery mnemonic and the CID of a previously delegated
/// authorization, revoke the authorization.
///
/// The callback arguments are (in order):
///
///  1. The context argument provided in the original call to
///     ns_sphere_authority_authorize
///  2. An owned pointer to an ns_error_t if there was an error, otherwise NULL
///
/// NOTE: The revocation must be performed by the root sphere credential
/// (derived from the recovery mnemonic), and the sphere revision that revokes
/// the authorization must be signed by same, so the revocation and a save is
/// all performed in one step by this function call. If there are pending writes
/// to the sphere, they will still be pending after this call, but any attempt
/// to save them will operate on the version of this sphere _after_ the
/// authorization is revoked.
pub fn ns_sphere_authority_authorization_revoke(
    noosphere: &NsNoosphere,
    sphere: &mut NsSphere,
    mnemonic: char_p::Ref<'_>,
    cid: char_p::Ref<'_>,
    context: Option<repr_c::Box<c_void>>,
    callback: extern "C" fn(Option<repr_c::Box<c_void>>, Option<repr_c::Box<NsError>>),
) {
    let mut sphere = sphere.inner_mut().clone();
    let mnemonic = Mnemonic(mnemonic.to_string());
    let cid = cid.to_string();
    let async_runtime = noosphere.async_runtime();

    noosphere.async_runtime().spawn(async move {
        let result = async {
            let authorization = Authorization::Cid(Cid::try_from(cid.as_str())?);

            sphere
                .with_root_authority(&mnemonic, move |mut root_sphere_context| async move {
                    root_sphere_context
                        .revoke_authorization(&authorization)
                        .await?;
                    Ok(Some(root_sphere_context.save(None).await?))
                })
                .await?;

            Ok(()) as Result<_, anyhow::Error>
        }
        .await;

        match result {
            Ok(_) => async_runtime.spawn_blocking(move || callback(context, None)),
            Err(error) => async_runtime.spawn_blocking(move || {
                callback(context, Some(NoosphereError::from(error).into()))
            }),
        };
    });
}
