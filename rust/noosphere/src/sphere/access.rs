use anyhow::Result;
use noosphere_core::authority::Authorization;
use ucan::crypto::KeyMaterial;

/// State that represents the form of access the application's user has to a
/// given sphere.
#[derive(Clone)]
pub enum SphereAccess<K>
where
    K: KeyMaterial + Clone + 'static,
{
    ReadOnly,
    ReadWrite {
        user_key: K,
        user_identity: String,
        authorization: Authorization,
    },
}

impl<K> SphereAccess<K>
where
    K: KeyMaterial + Clone + 'static,
{
    pub async fn read_write(user_key: K, authorization: Authorization) -> Result<Self> {
        let user_identity = user_key.get_did().await?;

        Ok(SphereAccess::ReadWrite {
            user_key,
            user_identity,
            authorization,
        })
    }
}
