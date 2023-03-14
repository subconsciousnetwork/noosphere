use anyhow::anyhow;
use cid::Cid;
use noosphere_core::{authority::Authorization, data::Did};
use noosphere_sphere::{HasSphereContext, SphereSync};
use safer_ffi::char_p::InvalidNulTerminator;
use safer_ffi::prelude::*;

use crate::ffi::{NsError, NsNoosphere, TryOrInitialize};
use crate::sphere::SphereReceipt;

#[derive_ReprC]
#[ReprC::opaque]
pub struct NsSphereReceipt {
    inner: SphereReceipt,
}

impl From<SphereReceipt> for NsSphereReceipt {
    fn from(inner: SphereReceipt) -> Self {
        NsSphereReceipt { inner }
    }
}

#[ffi_export]
/// Read the sphere identity (a DID encoded as a UTF-8 string) from a
/// [SphereReceipt]
pub fn ns_sphere_receipt_identity<'a>(
    sphere_receipt: &'a NsSphereReceipt,
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
/// Read the mnemonic from a [SphereReceipt]
pub fn ns_sphere_receipt_mnemonic<'a>(
    sphere_receipt: &'a NsSphereReceipt,
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
/// De-allocate a [SphereReceipt]
pub fn ns_sphere_receipt_free(sphere_receipt: repr_c::Box<NsSphereReceipt>) {
    drop(sphere_receipt)
}

#[ffi_export]
/// Initialize a brand new sphere, authorizing the given key to administer it.
/// The returned value is a [SphereReceipt], containing the DID of the sphere
/// and a human-readable mnemonic that can be used to rotate the key authorized
/// to administer the sphere.
pub fn ns_sphere_create(
    noosphere: &mut NsNoosphere,
    owner_key_name: char_p::Ref<'_>,
    error_out: Option<Out<'_, repr_c::Box<NsError>>>,
) -> Option<repr_c::Box<NsSphereReceipt>> {
    error_out.try_or_initialize(|| {
        Ok(repr_c::Box::new(
            noosphere
                .async_runtime()
                .block_on(noosphere.inner_mut().create_sphere(owner_key_name.to_str()))
                .map(|receipt| receipt.into())?,
        ))
    })
}

#[ffi_export]
/// Join a sphere by initializing it and configuring it to use the specified
/// key and authorization. The authorization should be provided in the form of
/// a base64-encoded CID v1 string.
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
            .block_on(noosphere.inner_mut().join_sphere(
                &Did::from(sphere_identity.to_str()),
                local_key_name.to_str(),
                Some(&authorization),
            ))
            .map_err(|error| error.into())
    });
}

#[ffi_export]
/// Get the version of a given sphere that is considered the most recent version
/// in local history. If a version is recorded, it is returned as a
/// base64-encoded CID v1 string.
pub fn ns_sphere_version_get(
    noosphere: &NsNoosphere,
    sphere_identity: char_p::Ref<'_>,
    error_out: Option<Out<'_, repr_c::Box<NsError>>>,
) -> Option<char_p::Box> {
    error_out.try_or_initialize(|| {
        noosphere.async_runtime().block_on(async {
            let sphere_context = noosphere
                .inner()
                .get_sphere_context(&Did(sphere_identity.to_str().into()))
                .await?;

            let sphere_context = sphere_context.lock().await;
            sphere_context
                .sphere()
                .await?
                .cid()
                .to_string()
                .try_into()
                .map_err(|error: InvalidNulTerminator<String>| anyhow!(error).into())
        })
    })
}

#[ffi_export]
/// Sync a sphere with a gateway. A gateway URL must have been configured when
/// the [NoosphereContext] was initialized. And, the sphere must have already
/// been created or joined by the caller so that it is locally initialized (it's
/// okay if this was done in an earlier session). The returned string is the
/// base64-encoded CID v1 of the latest locally-available sphere revision after
/// the synchronization process has successfully completed.
pub fn ns_sphere_sync(
    noosphere: &mut NsNoosphere,
    sphere_identity: char_p::Ref<'_>,
    error_out: Option<Out<'_, repr_c::Box<NsError>>>,
) -> Option<char_p::Box> {
    error_out.try_or_initialize(|| {
        let cid = noosphere.async_runtime().block_on(async {
            let mut sphere_context = noosphere
                .inner()
                .get_sphere_context(&Did(sphere_identity.to_str().into()))
                .await?;

            sphere_context.sync().await?;

            Ok(sphere_context.to_sphere().await?.cid().to_string()) as Result<String, anyhow::Error>
        })?;

        Ok(cid
            .try_into()
            .map_err(|error: InvalidNulTerminator<String>| anyhow!(error))?)
    })
}
