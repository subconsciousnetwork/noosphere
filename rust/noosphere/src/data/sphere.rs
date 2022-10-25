use cid::Cid;
use serde::{Deserialize, Serialize};

/// The root of the sphere, containing pointers to public details such as names
/// and links, as well as "sealed" (private) data. While public details are accessible
/// to all, sealed data is encrypted at rest and only accessible to the user who
/// owns the sphere.
#[derive(Default, Debug, Eq, PartialEq, Clone, Serialize, Deserialize)]
pub struct SphereIpld {
    /// A DID that is the identity of the originating key that owns the sphere
    pub identity: String,

    /// The public links for the sphere (LinksIpld)
    pub links: Option<Cid>,

    /// The non-public content of the sphere (SealedIpld)
    pub sealed: Option<Cid>,

    /// Authorization and revocation state for non-owner keys (AuthorizationIpld)
    pub authorization: Option<Cid>,
}

impl SphereIpld {}

#[cfg(test)]
mod tests {
    use ed25519_zebra::{SigningKey as Ed25519PrivateKey, VerificationKey as Ed25519PublicKey};
    use libipld_cbor::DagCborCodec;
    use ucan::{
        builder::UcanBuilder,
        capability::{Capability, Resource, With},
        crypto::{did::DidParser, KeyMaterial},
        store::UcanJwtStore,
    };
    use ucan_key_support::ed25519::Ed25519KeyMaterial;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    use crate::{
        authority::{
            verify_sphere_cid, Authorization, SphereAction, SphereReference, SUPPORTED_KEYS,
        },
        data::{ContentType, Header, MemoIpld, SphereIpld},
    };

    use noosphere_storage::{db::SphereDb, interface::BlockStore, memory::MemoryStorageProvider};

    fn generate_credential() -> Ed25519KeyMaterial {
        let private_key = Ed25519PrivateKey::new(rand::thread_rng());
        let public_key = Ed25519PublicKey::from(&private_key);
        Ed25519KeyMaterial(public_key, Some(private_key))
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_be_signed_by_identity_key_and_verified() {
        let identity_credential = generate_credential();
        let identity_did = identity_credential.get_did().await.unwrap();

        let mut store = SphereDb::new(&MemoryStorageProvider::default())
            .await
            .unwrap();

        let sphere = SphereIpld {
            identity: identity_did.clone(),
            links: None,
            sealed: None,
            authorization: None,
        };

        let sphere_cid = store.save::<DagCborCodec, _>(&sphere).await.unwrap();

        let mut memo = MemoIpld {
            parent: None,
            headers: vec![(
                Header::ContentType.to_string(),
                ContentType::Sphere.to_string(),
            )],
            body: sphere_cid,
        };

        let capability: Capability<SphereReference, SphereAction> = Capability {
            with: With::Resource {
                kind: Resource::Scoped(SphereReference {
                    did: identity_did.clone(),
                }),
            },
            can: SphereAction::Authorize,
        };

        let authorization = Authorization::Ucan(
            UcanBuilder::default()
                .issued_by(&identity_credential)
                .for_audience(&identity_did)
                .with_lifetime(100)
                .claiming_capability(&capability)
                .build()
                .unwrap()
                .sign()
                .await
                .unwrap(),
        );

        memo.sign(&identity_credential, Some(&authorization))
            .await
            .unwrap();

        store
            .write_token(
                &authorization
                    .resolve_ucan(&store)
                    .await
                    .unwrap()
                    .encode()
                    .unwrap(),
            )
            .await
            .unwrap();

        let memo_cid = store.save::<DagCborCodec, _>(&memo).await.unwrap();

        let mut did_parser = DidParser::new(SUPPORTED_KEYS);

        verify_sphere_cid(&memo_cid, &store, &mut did_parser)
            .await
            .unwrap();
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_be_signed_by_an_authorized_key_and_verified() {
        let identity_credential = generate_credential();
        let authorized_credential = generate_credential();

        let identity_did = identity_credential.get_did().await.unwrap();
        let authorized_did = authorized_credential.get_did().await.unwrap();

        let mut store = SphereDb::new(&MemoryStorageProvider::default())
            .await
            .unwrap();

        let sphere = SphereIpld {
            identity: identity_did.clone(),
            links: None,
            sealed: None,
            authorization: None,
        };

        let sphere_cid = store.save::<DagCborCodec, _>(&sphere).await.unwrap();

        let mut memo = MemoIpld {
            parent: None,
            headers: vec![(
                Header::ContentType.to_string(),
                ContentType::Sphere.to_string(),
            )],
            body: sphere_cid,
        };

        let capability: Capability<SphereReference, SphereAction> = Capability {
            with: With::Resource {
                kind: Resource::Scoped(SphereReference {
                    did: identity_did.clone(),
                }),
            },
            can: SphereAction::Authorize,
        };

        let authorization = Authorization::Ucan(
            UcanBuilder::default()
                .issued_by(&identity_credential)
                .for_audience(&authorized_did)
                .with_lifetime(100)
                .claiming_capability(&capability)
                .build()
                .unwrap()
                .sign()
                .await
                .unwrap(),
        );

        memo.sign(&authorized_credential, Some(&authorization))
            .await
            .unwrap();

        store
            .write_token(
                &authorization
                    .resolve_ucan(&store)
                    .await
                    .unwrap()
                    .encode()
                    .unwrap(),
            )
            .await
            .unwrap();

        let memo_cid = store.save::<DagCborCodec, _>(&memo).await.unwrap();

        let mut did_parser = DidParser::new(SUPPORTED_KEYS);

        verify_sphere_cid(&memo_cid, &store, &mut did_parser)
            .await
            .unwrap();
    }
}
