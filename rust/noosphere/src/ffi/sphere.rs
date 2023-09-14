use std::ffi::c_void;

use anyhow::anyhow;
use cid::Cid;
use noosphere_core::context::{HasSphereContext, SphereSync};
use noosphere_core::data::Mnemonic;
use noosphere_core::{authority::Authorization, data::Did};
use safer_ffi::char_p::InvalidNulTerminator;
use safer_ffi::prelude::*;

use crate::ffi::{NsError, NsNoosphere, NsSphere, TryOrInitialize};
use crate::sphere::SphereReceipt;

#[derive_ReprC(rename = "ns_sphere_receipt")]
#[repr(opaque)]
/// @class ns_sphere_receipt_t
/// An opaque struct representing initialization information of a sphere.
///
/// Contains the identity of a sphere (DID) and a human-readable
/// mnemonic that can be used to rotate the key authorized
/// to administer the sphere.
pub struct NsSphereReceipt {
    inner: SphereReceipt,
}

impl From<SphereReceipt> for NsSphereReceipt {
    fn from(inner: SphereReceipt) -> Self {
        NsSphereReceipt { inner }
    }
}

#[ffi_export]
/// @memberof ns_sphere_receipt_t
/// Read the sphere identity (a DID encoded as a UTF-8 string) from a
/// ns_sphere_receipt_t
pub fn ns_sphere_receipt_identity(
    sphere_receipt: &NsSphereReceipt,
    error_out: Option<Out<'_, repr_c::Box<NsError>>>,
) -> Option<char_p::Box> {
    error_out.try_or_initialize(|| {
        sphere_receipt
            .inner
            .identity
            .to_string()
            .try_into()
            .map_err(|error: InvalidNulTerminator<String>| anyhow!(error).into())
    })
}

#[ffi_export]
/// @memberof ns_sphere_receipt_t
/// Read the mnemonic from a ns_sphere_receipt_t.
pub fn ns_sphere_receipt_mnemonic(
    sphere_receipt: &NsSphereReceipt,
    error_out: Option<Out<'_, repr_c::Box<NsError>>>,
) -> Option<char_p::Box> {
    error_out.try_or_initialize(|| {
        sphere_receipt
            .inner
            .mnemonic
            .to_string()
            .try_into()
            .map_err(|error: InvalidNulTerminator<String>| anyhow!(error).into())
    })
}

#[ffi_export]
/// @memberof ns_sphere_receipt_t
/// Deallocate a ns_sphere_receipt_t
pub fn ns_sphere_receipt_free(sphere_receipt: repr_c::Box<NsSphereReceipt>) {
    drop(sphere_receipt)
}

#[ffi_export]
/// @memberof ns_noosphere_t
/// Initialize a brand new sphere, authorizing the given key to administer it.
///
/// The returned value is a ns_sphere_receipt_t, containing the DID of the sphere
/// and a human-readable mnemonic that can be used to rotate the key authorized
/// to administer the sphere.
pub fn ns_sphere_create(
    noosphere: &mut NsNoosphere,
    owner_key_name: char_p::Ref<'_>,
    error_out: Option<Out<'_, repr_c::Box<NsError>>>,
) -> Option<repr_c::Box<NsSphereReceipt>> {
    error_out.try_or_initialize(|| {
        Ok(Box::<NsSphereReceipt>::new(
            noosphere
                .async_runtime()
                .block_on(noosphere.inner().create_sphere(owner_key_name.to_str()))
                .map(|receipt| receipt.into())?,
        )
        .into())
    })
}

#[ffi_export]
/// @memberof ns_noosphere_t
///
/// Join a sphere by initializing it and configuring it to use the specified key
/// and authorization.
///
/// The authorization should be provided in the form of a base64-encoded CID v1
/// string.
pub fn ns_sphere_join(
    noosphere: &mut NsNoosphere,
    sphere_identity: char_p::Ref<'_>,
    local_key_name: char_p::Ref<'_>,
    authorization: char_p::Ref<'_>,
    error_out: Option<Out<'_, repr_c::Box<NsError>>>,
) {
    error_out.try_or_initialize(|| {
        let authorization = Authorization::Cid(
            Cid::try_from(authorization.to_str()).map_err(|error| anyhow!(error))?,
        );
        noosphere
            .async_runtime()
            .block_on(noosphere.inner().join_sphere(
                &Did::from(sphere_identity.to_str()),
                local_key_name.to_str(),
                Some(&authorization),
            ))
            .map_err(|error| error.into())
    });
}

#[ffi_export]
/// @memberof ns_sphere_t
/// @deprecated Use ns_sphere_version instead
///
/// Get the version of a given sphere that is considered the most recent version
/// in local history. NOTE: This only works for spheres that were initialized
/// locally (which is to say, origin spheres).
///
/// If a version is recorded, it is returned as a base64-encoded CID v1 string.
pub fn ns_sphere_version_get(
    noosphere: &NsNoosphere,
    sphere_identity: char_p::Ref<'_>,
    error_out: Option<Out<'_, repr_c::Box<NsError>>>,
) -> Option<char_p::Box> {
    error_out.try_or_initialize(|| {
        noosphere.async_runtime().block_on(async {
            let sphere_channel = noosphere
                .inner()
                .get_sphere_channel(&Did(sphere_identity.to_str().into()))
                .await?;

            let sphere_context = sphere_channel.immutable();
            sphere_context
                .to_sphere()
                .await?
                .cid()
                .to_string()
                .try_into()
                .map_err(|error: InvalidNulTerminator<String>| anyhow!(error).into())
        })
    })
}

#[ffi_export]
/// @memberof ns_sphere_t
///
/// Sync a sphere with a gateway.
///
/// A gateway URL must have been configured when the ns_noosphere_t was
/// initialized. And, the sphere must have already been created or joined by the
/// caller so that it is locally initialized (it's okay if this was done in an
/// earlier session). The returned string is the base64-encoded CID v1 of the
/// latest locally-available sphere revision after the synchronization process
/// has successfully completed.
///
/// The callback arguments are (in order):
///
///  1. The context argument provided in the original call to
///     ns_sphere_file_contents_read
///  2. An owned pointer to an ns_error_t if there was an error, otherwise NULL
///  3. An owned pointer to a null terminated UTF-8 string if the call was
///     successful, otherwise NULL
///
pub fn ns_sphere_sync(
    noosphere: &NsNoosphere,
    sphere: &NsSphere,
    context: Option<repr_c::Box<c_void>>,
    callback: extern "C" fn(
        Option<repr_c::Box<c_void>>,
        Option<repr_c::Box<NsError>>,
        Option<char_p::Box>,
    ),
) {
    let mut sphere_channel = sphere.to_channel();

    noosphere.async_runtime().spawn(async move {
        let result: Result<char_p::Box, anyhow::Error> = async {
            sphere_channel.mutable().sync().await?;

            sphere_channel
                .immutable()
                .to_sphere()
                .await?
                .cid()
                .to_string()
                .try_into()
                .map_err(|error: InvalidNulTerminator<String>| anyhow!(error))
        }
        .await;

        match result {
            Ok(cid_string) => {
                tokio::task::spawn_blocking(move || callback(context, None, Some(cid_string)))
            }
            Err(error) => tokio::task::spawn_blocking(move || {
                callback(context, Some(NsError::from(error).into()), None)
            }),
        };
    });
}

#[ffi_export]
/// @memberof ns_sphere_t
///
/// @deprecated Blocking FFI is deprecated, use ns_sphere_sync_ instead
///
/// Same as ns_sphere_sync, but blocks the current thread while performing its
/// work.
pub fn ns_sphere_sync_blocking(
    noosphere: &NsNoosphere,
    sphere_identity: char_p::Ref<'_>,
    error_out: Option<Out<'_, repr_c::Box<NsError>>>,
) -> Option<char_p::Box> {
    error_out.try_or_initialize(|| {
        let cid = noosphere.async_runtime().block_on(async {
            let mut sphere_channel = noosphere
                .inner()
                .get_sphere_channel(&Did(sphere_identity.to_str().into()))
                .await?;

            sphere_channel.mutable().sync().await?;

            Ok(sphere_channel
                .immutable()
                .to_sphere()
                .await?
                .cid()
                .to_string()) as Result<String, anyhow::Error>
        })?;

        Ok(cid
            .try_into()
            .map_err(|error: InvalidNulTerminator<String>| anyhow!(error))?)
    })
}

#[ffi_export]
/// @memberof ns_noosphere_t
///
/// Recover a sphere by fetching is history from a gateway.
///
/// This is intended to be used in cases when local data has been corrupted or
/// is otherwise unavailable. If the user knows their gateway URL, their sphere
/// ID and has their mnemonic handy, they can exchange these pieces of
/// information to perform a recovery operation. The existing block storage
/// layer is backed up and a new one is initialized and populated from the
/// gateway.
///
/// The callback arguments are (in order):
///
///  1. The context argument provided in the original call to ns_sphere_recover
///  2. An owned pointer to an ns_error_t if there was an error, otherwise NULL
///
pub fn ns_sphere_recover(
    noosphere: &NsNoosphere,
    sphere_identity: char_p::Ref<'_>,
    local_key_name: char_p::Ref<'_>,
    mnemonic: char_p::Ref<'_>,
    context: Option<repr_c::Box<c_void>>,
    callback: extern "C" fn(Option<repr_c::Box<c_void>>, Option<repr_c::Box<NsError>>),
) {
    let noosphere_inner = noosphere.inner().clone();
    let sphere_identity = Did(sphere_identity.to_string());
    let mnemonic = Mnemonic(mnemonic.to_string());
    let local_key_name = local_key_name.to_string();

    noosphere.async_runtime().spawn(async move {
        let result = noosphere_inner
            .recover_sphere(&local_key_name, &sphere_identity, &mnemonic)
            .await;

        match result {
            Ok(_) => tokio::task::spawn_blocking(move || callback(context, None)),
            Err(error) => tokio::task::spawn_blocking(move || {
                callback(context, Some(NsError::from(error).into()))
            }),
        };
    });
}
