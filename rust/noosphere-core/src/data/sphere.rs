use anyhow::Result;
use cid::Cid;
use libipld_cbor::DagCborCodec;
use noosphere_storage::BlockStore;
use serde::{Deserialize, Serialize};

use super::{AddressBookIpld, AuthorityIpld, ContentIpld, Did, Link};

/// The root of the sphere, containing pointers to public details such as names
/// and links, as well as "sealed" (private) data. While public details are accessible
/// to all, sealed data is encrypted at rest and only accessible to the user who
/// owns the sphere.
#[derive(Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub struct SphereIpld {
    /// A DID that is the identity of the originating key that owns the sphere
    pub identity: Did,

    /// The public links for the sphere
    pub content: Link<ContentIpld>,

    /// The public pet names for the sphere
    pub address_book: Link<AddressBookIpld>,

    /// Authorization and revocation state for non-owner keys
    pub authority: Link<AuthorityIpld>,

    /// The non-public content of the sphere
    pub private: Option<Cid>,
}

impl SphereIpld {
    /// Initialize a new, empty [SphereIpld] with a given [Did] sphere identity.
    pub async fn new<S>(identity: &Did, store: &mut S) -> Result<SphereIpld>
    where
        S: BlockStore,
    {
        let content_ipld = ContentIpld::empty(store).await?;
        let content = store.save::<DagCborCodec, _>(&content_ipld).await?.into();

        let address_book_ipld = AddressBookIpld::empty(store).await?;
        let address_book = store
            .save::<DagCborCodec, _>(&address_book_ipld)
            .await?
            .into();

        let authority_ipld = AuthorityIpld::empty(store).await?;
        let authority = store.save::<DagCborCodec, _>(&authority_ipld).await?.into();

        Ok(SphereIpld {
            identity: identity.clone(),
            content,
            address_book,
            authority,
            private: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use libipld_cbor::DagCborCodec;
    use ucan::{builder::UcanBuilder, crypto::KeyMaterial, store::UcanJwtStore};
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    use crate::{
        authority::{generate_capability, generate_ed25519_key, SphereAbility},
        data::{ContentType, Did, Header, MemoIpld, SphereIpld},
        view::Sphere,
    };

    use noosphere_storage::{BlockStore, MemoryStorage, SphereDb};

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_be_signed_by_identity_key_and_verified() -> Result<()> {
        let identity_credential = generate_ed25519_key();
        let identity = Did(identity_credential.get_did().await?);

        let mut store = SphereDb::new(MemoryStorage::default()).await?;

        let sphere = SphereIpld::new(&identity, &mut store).await?;

        let sphere_cid = store.save::<DagCborCodec, _>(&sphere).await?;

        let mut memo = MemoIpld {
            parent: None,
            headers: vec![(
                Header::ContentType.to_string(),
                ContentType::Sphere.to_string(),
            )],
            body: sphere_cid,
        };

        let capability = generate_capability(&identity, SphereAbility::Authorize);

        let authorization = UcanBuilder::default()
            .issued_by(&identity_credential)
            .for_audience(&identity)
            .with_lifetime(100)
            .claiming_capability(&capability)
            .build()?
            .sign()
            .await?;

        memo.sign(&identity_credential, Some(&authorization))
            .await?;

        store.write_token(&authorization.encode()?).await?;

        let memo_cid = store.save::<DagCborCodec, _>(&memo).await?.into();

        let sphere = Sphere::at(&memo_cid, &store);

        sphere.verify_signature().await?;

        Ok(())
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_be_signed_by_an_authorized_key_and_verified() -> Result<()> {
        let identity_credential = generate_ed25519_key();
        let authorized_credential = generate_ed25519_key();

        let identity = Did(identity_credential.get_did().await?);
        let authorized = Did(authorized_credential.get_did().await?);

        let mut store = SphereDb::new(MemoryStorage::default()).await?;

        let sphere = SphereIpld::new(&identity, &mut store).await?;

        let sphere_cid = store.save::<DagCborCodec, _>(&sphere).await?;

        let mut memo = MemoIpld {
            parent: None,
            headers: vec![(
                Header::ContentType.to_string(),
                ContentType::Sphere.to_string(),
            )],
            body: sphere_cid,
        };

        let capability = generate_capability(&identity, SphereAbility::Authorize);
        let authorization = UcanBuilder::default()
            .issued_by(&identity_credential)
            .for_audience(&authorized)
            .with_lifetime(100)
            .claiming_capability(&capability)
            .build()?
            .sign()
            .await?;

        memo.sign(&authorized_credential, Some(&authorization))
            .await?;

        store.write_token(&authorization.encode().unwrap()).await?;

        let memo_cid = store.save::<DagCborCodec, _>(&memo).await?.into();

        let sphere = Sphere::at(&memo_cid, &store);

        sphere.verify_signature().await?;

        Ok(())
    }
}
