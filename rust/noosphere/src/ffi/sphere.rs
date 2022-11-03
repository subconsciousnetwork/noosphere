use cid::Cid;
use noosphere_core::authority::Authorization;
use safer_ffi::prelude::*;

use crate::ffi::NoosphereContext;
use crate::sphere::SphereReceipt as SphereReceiptImpl;

ReprC! {
    #[ReprC::opaque]
    pub struct SphereReceipt {
        inner: SphereReceiptImpl
    }
}

impl From<SphereReceiptImpl> for SphereReceipt {
    fn from(inner: SphereReceiptImpl) -> Self {
        SphereReceipt { inner }
    }
}

#[ffi_export]
/// Read the sphere identity (a DID encoded as a UTF-8 string) from a
/// [SphereReceipt]
pub fn noosphere_sphere_receipt_identity<'a>(
    sphere_receipt: &'a repr_c::Box<SphereReceipt>,
) -> char_p::Ref<'a> {
    char_p::Ref::try_from(sphere_receipt.inner.identity.as_str()).unwrap()
}

#[ffi_export]
/// Read the mnemonic from a [SphereReceipt]
pub fn noosphere_sphere_receipt_mnemonic<'a>(
    sphere_receipt: &'a repr_c::Box<SphereReceipt>,
) -> char_p::Ref<'a> {
    char_p::Ref::try_from(sphere_receipt.inner.mnemonic.as_str()).unwrap()
}

#[ffi_export]
/// De-allocate a [SphereReceipt]
pub fn noosphere_free_sphere_receipt(sphere_receipt: repr_c::Box<SphereReceipt>) {
    drop(sphere_receipt)
}

#[ffi_export]
/// Initialize a brand new sphere, authorizing the given key to administer it.
/// The returned value is a [SphereReceipt], containing the DID of the sphere
/// and a human-readable mnemonic that can be used to rotate the key authorized
/// to administer the sphere.
pub fn noosphere_create_sphere(
    noosphere: &mut repr_c::Box<NoosphereContext>,
    owner_key_name: char_p::Ref<'_>,
) -> repr_c::Box<SphereReceipt> {
    repr_c::Box::new(
        pollster::block_on(noosphere.inner_mut().create_sphere(owner_key_name.to_str()))
            .unwrap()
            .into(),
    )
}

#[ffi_export]
/// Join a sphere by initializing it and configuring it to use the specified
/// key and authorization. The authorization should be provided in the form of
/// a base64-encoded CID v1 string.
pub fn noosphere_join_sphere(
    noosphere: &mut repr_c::Box<NoosphereContext>,
    sphere_identity: char_p::Ref<'_>,
    local_key_name: char_p::Ref<'_>,
    authorization: char_p::Ref<'_>,
) {
    let authorization = Authorization::Cid(Cid::try_from(authorization.to_str()).unwrap());
    pollster::block_on(noosphere.inner_mut().join_sphere(
        sphere_identity.to_str(),
        local_key_name.to_str(),
        &authorization,
    ))
    .unwrap();
}
