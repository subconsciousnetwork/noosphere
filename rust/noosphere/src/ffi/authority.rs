use anyhow::anyhow;
use cid::Cid;
use noosphere_core::{
    authority::Authorization,
    data::{Did, Jwt, Link, Mnemonic},
};
use noosphere_sphere::{HasSphereContext, SphereAuthorityRead, SphereAuthorityWrite, SphereWalker};
use safer_ffi::{char_p::InvalidNulTerminator, prelude::*};
use std::ffi::c_void;

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
/// Given the CID of a previously delegated authorization, revoke the
/// authorization.
///
/// The callback arguments are (in order):
///
///  1. The context argument provided in the original call to
///     ns_sphere_authority_authorize
///  2. An owned pointer to an ns_error_t if there was an error, otherwise NULL
pub fn ns_sphere_authority_authorization_revoke(
    noosphere: &NsNoosphere,
    sphere: &mut NsSphere,
    cid: char_p::Ref<'_>,
    context: Option<repr_c::Box<c_void>>,
    callback: extern "C" fn(Option<repr_c::Box<c_void>>, Option<repr_c::Box<NsError>>),
) {
    let mut sphere = sphere.inner_mut().clone();
    let cid = cid.to_string();
    let async_runtime = noosphere.async_runtime();

    noosphere.async_runtime().spawn(async move {
        let result = async {
            let authorization = Authorization::Cid(Cid::try_from(cid.as_str())?);

            sphere.revoke_authorization(&authorization).await?;

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

#[ffi_export]
/// @memberof ns_sphere_t
///
/// List all of the authorizations. Authorizations are given as an array of
/// base64-encoded CIDv1 strings. The name and/or authorized DID of each
/// authorization can by looked up by futher calls to
/// ns_sphere_authority_authorization_name and/or
/// ns_sphere_authority_authorization_identity respectively.
///
/// The callback arguments are (in order):
///
///  1. The context argument provided in the original call to
///     ns_sphere_authority_authorize
///  2. An owned pointer to an ns_error_t if there was an error, otherwise NULL
///  3. An owned pointer to a slice_boxed_char_ptr_t
pub fn ns_sphere_authority_authorizations_list(
    noosphere: &NsNoosphere,
    sphere: &NsSphere,
    context: Option<repr_c::Box<c_void>>,
    callback: extern "C" fn(
        Option<repr_c::Box<c_void>>,
        Option<repr_c::Box<NsError>>,
        c_slice::Box<char_p::Box>,
    ),
) {
    let sphere = sphere.inner().clone();
    let async_runtime = noosphere.async_runtime();

    noosphere.async_runtime().spawn(async move {
        let result = async {
            let authorizations = SphereWalker::from(&sphere).list_authorizations().await?;

            let mut all_authorizations: Vec<char_p::Box> = Vec::new();

            for authorization in authorizations.into_iter() {
                all_authorizations.push(
                    authorization
                        .to_string()
                        .try_into()
                        .map_err(|error: InvalidNulTerminator<String>| anyhow!(error))?,
                )
            }

            Ok(all_authorizations.into_boxed_slice().into())
                as Result<c_slice::Box<char_p::Box>, anyhow::Error>
        }
        .await;

        match result {
            Ok(authorizations) => {
                async_runtime.spawn_blocking(move || callback(context, None, authorizations))
            }
            Err(error) => async_runtime.spawn_blocking(move || {
                callback(
                    context,
                    Some(NoosphereError::from(error).into()),
                    Vec::new().into_boxed_slice().into(),
                )
            }),
        };
    });
}

/// @memberof ns_sphere_t
///
/// Get the name associated with the given authorization. The authorization
/// should be provided as a base64-encoded CIDv1 string.
///
/// The callback arguments are (in order):
///
///  1. The context argument provided in the original call to
///     ns_sphere_authority_authorize
///  2. An owned pointer to an ns_error_t if there was an error, otherwise NULL
///  3. An owned pointer to constant string (the name of the authorization) if
///     the call was successful, otherwise NULL
pub fn ns_sphere_authority_authorization_name(
    noosphere: &NsNoosphere,
    sphere: &NsSphere,
    authorization: char_p::Ref<'_>,
    context: Option<repr_c::Box<c_void>>,
    callback: extern "C" fn(
        Option<repr_c::Box<c_void>>,
        Option<repr_c::Box<NsError>>,
        Option<char_p::Box>,
    ),
) {
    let sphere = sphere.inner().clone();
    let authorization = authorization.to_string();
    let async_runtime = noosphere.async_runtime();

    noosphere.async_runtime().spawn(async move {
        let result = async {
            let link = Link::<Jwt>::from(Cid::try_from(authorization)?);

            let name: char_p::Box = sphere
                .to_sphere()
                .await?
                .get_authority()
                .await?
                .get_delegations()
                .await?
                .require(&link)
                .await?
                .name
                .clone()
                .try_into()?;

            Ok(name) as Result<_, anyhow::Error>
        }
        .await;

        match result {
            Ok(name) => async_runtime.spawn_blocking(move || callback(context, None, Some(name))),
            Err(error) => async_runtime.spawn_blocking(move || {
                callback(context, Some(NoosphereError::from(error).into()), None)
            }),
        };
    });
}

/// @memberof ns_sphere_t
///
/// Get the DID associated with the given authorization. The authorization should be
/// provided as a base64-encoded CIDv1 string.
///
/// The callback arguments are (in order):
///
///  1. The context argument provided in the original call to
///     ns_sphere_authority_authorize
///  2. An owned pointer to an ns_error_t if there was an error, otherwise NULL
///  3. An owned pointer to constant string (the authorized DID) if the call
///     was successful, otherwise NULL
pub fn ns_sphere_authority_authorization_identity(
    noosphere: &NsNoosphere,
    sphere: &NsSphere,
    authorization: char_p::Ref<'_>,
    context: Option<repr_c::Box<c_void>>,
    callback: extern "C" fn(
        Option<repr_c::Box<c_void>>,
        Option<repr_c::Box<NsError>>,
        Option<char_p::Box>,
    ),
) {
    let sphere = sphere.inner().clone();
    let authorization = authorization.to_string();
    let async_runtime = noosphere.async_runtime();

    noosphere.async_runtime().spawn(async move {
        let result = async {
            let link = Link::<Jwt>::from(Cid::try_from(authorization)?);

            let ucan = Authorization::Cid(link.into())
                .as_ucan(sphere.sphere_context().await?.db())
                .await?;

            let identity: char_p::Box = ucan.audience().to_string().try_into()?;

            Ok(identity) as Result<_, anyhow::Error>
        }
        .await;

        match result {
            Ok(identity) => {
                async_runtime.spawn_blocking(move || callback(context, None, Some(identity)))
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
/// Given a sphere root credential recovery mnemonic, get back a ns_sphere_t
/// with an escalated credential that allows signing changes as the sphere
/// itself.
///
/// This function call will return an error if the mnemonic is invalid or if the
/// credential it represents is not the sphere's key.
///
/// The callback arguments are (in order):
///
///  1. The context argument provided in the original call to
///     ns_sphere_authority_authorize
///  2. An owned pointer to an ns_error_t if there was an error, otherwise NULL
///  3. An owned pointer to an ns_sphere_t with the root sphere credential,
///     otherwise NULL
#[allow(clippy::type_complexity)]
pub fn ns_sphere_authority_escalate(
    noosphere: &NsNoosphere,
    sphere: &mut NsSphere,
    mnemonic: char_p::Ref<'_>,
    context: Option<repr_c::Box<c_void>>,
    callback: extern "C" fn(
        Option<repr_c::Box<c_void>>,
        Option<repr_c::Box<NsError>>,
        Option<repr_c::Box<NsSphere>>,
    ),
) {
    let sphere = sphere.inner().clone();
    let mnemonic = Mnemonic(mnemonic.to_string());
    let async_runtime = noosphere.async_runtime();

    noosphere.async_runtime().spawn(async move {
        let result = async {
            let root_sphere_context = sphere.escalate_authority(&mnemonic).await?;

            Ok(Box::new(NsSphere {
                inner: root_sphere_context.into(),
            })
            .into()) as Result<repr_c::Box<NsSphere>, anyhow::Error>
        }
        .await;

        match result {
            Ok(sphere) => {
                async_runtime.spawn_blocking(move || callback(context, None, Some(sphere)))
            }
            Err(error) => async_runtime.spawn_blocking(move || {
                callback(context, Some(NoosphereError::from(error).into()), None)
            }),
        };
    });
}
