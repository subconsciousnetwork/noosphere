use crate::{
    authority::{
        generate_ed25519_key, Authorization, SphereAction, SphereReference, SPHERE_SEMANTICS,
        SUPPORTED_KEYS,
    },
    data::Did,
};
use anyhow::{anyhow, Result};
use cid::Cid;
use noosphere_storage::{SphereDb, Storage};
use serde_json;
use ucan::{
    builder::UcanBuilder,
    capability::{Capability, Resource, With},
    chain::ProofChain,
    crypto::{did::DidParser, KeyMaterial},
    Ucan,
};
use ucan_key_support::ed25519::Ed25519KeyMaterial;

/// The level of access that a given user has to a related resource. Broadly,
/// a user will always have either read/write access (to their own sphere) or
/// else read-only access (to all other spheres).
#[derive(PartialEq, Eq, Debug, Clone)]
pub enum Access {
    ReadWrite,
    ReadOnly,
}

/// An author is a user or program who is reading content from and/or writing
/// content to a sphere. This construct collects the identity and the
/// authorization of that entity to make it easier to determine their level of
/// access to the content of a given sphere.
#[derive(Clone)]
pub struct Author<K>
where
    K: KeyMaterial + Clone + 'static,
{
    pub key: K,
    pub authorization: Option<Authorization>,
}

impl Author<Ed25519KeyMaterial> {
    /// Produces an "anonymous" author who is guaranteed not to have any
    /// authorization assigned to it prior to being created
    pub fn anonymous() -> Self {
        Author {
            key: generate_ed25519_key(),
            authorization: None,
        }
    }
}

impl<K> Author<K>
where
    K: KeyMaterial + Clone + 'static,
{
    /// Resolve the identity of the author
    pub async fn identity(&self) -> Result<Did> {
        Ok(Did(self.key.get_did().await?))
    }

    /// For cases where some kind of authorization is expected, this accessor
    /// can be used to automatically produce an error result if the
    /// authorization is not present
    pub fn require_authorization(&self) -> Result<&Authorization> {
        self.authorization
            .as_ref()
            .ok_or_else(|| anyhow!("Authorization is required but none is configured"))
    }

    /// Determine the level of access that the author has to a given sphere
    pub async fn access_to<S: Storage>(
        &self,
        sphere_identity: &Did,
        db: &SphereDb<S>,
    ) -> Result<Access> {
        if let Some(authorization) = &self.authorization {
            let author_did = Did(self.key.get_did().await?);
            let ucan = authorization.resolve_ucan(db).await?;

            if ucan.audience() != author_did.as_str() {
                return Ok(Access::ReadOnly);
            }

            let read_write_capability = Capability {
                with: With::Resource {
                    kind: Resource::Scoped(SphereReference {
                        did: sphere_identity.to_string(),
                    }),
                },
                can: SphereAction::Push,
            };
            let mut did_parser = DidParser::new(SUPPORTED_KEYS);
            let proof_chain = ProofChain::from_ucan(ucan, &mut did_parser, db).await?;

            let capability_infos = proof_chain.reduce_capabilities(&SPHERE_SEMANTICS);

            for info in capability_infos {
                if info.originators.contains(sphere_identity.as_str())
                    && info.capability.enables(&read_write_capability)
                {
                    return Ok(Access::ReadWrite);
                }
            }
        }

        Ok(Access::ReadOnly)
    }

    /// A helper for creating and signing a new UCAN token for publishing.
    pub async fn create_publish_token(
        &self,
        sphere_id: &str,
        address: &Cid,
        authority: &Cid,
        lifetime: u64,
    ) -> Result<Ucan> {
        let mut signable = UcanBuilder::default()
            .issued_by(&self.key)
            .for_audience(sphere_id)
            .with_lifetime(lifetime)
            .claiming_capability(&Capability {
                with: With::Resource {
                    kind: Resource::Scoped(SphereReference {
                        did: sphere_id.to_owned(),
                    }),
                },
                can: SphereAction::Publish,
            })
            .with_fact(serde_json::json!({ "link": address.to_string() }))
            .build()?;

        // TODO(ucan-wg/rs-ucan#32): This is kind of a hack until we can add proofs by CID
        signable.proofs.push(authority.to_string());
        signable.sign().await
    }
}

#[cfg(test)]
mod tests {
    use noosphere_storage::{MemoryStorage, SphereDb};
    use ucan::crypto::KeyMaterial;

    use crate::{authority::generate_ed25519_key, data::Did, view::Sphere};

    use super::{Access, Author};

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_gives_read_only_access_when_there_is_no_authorization() {
        let author = Author::anonymous();
        let mut db = SphereDb::new(&MemoryStorage::default()).await.unwrap();

        let (sphere, _, _) = Sphere::try_generate("did:key:foo", &mut db).await.unwrap();

        let access = author
            .access_to(&sphere.try_get_identity().await.unwrap(), &db)
            .await
            .unwrap();

        assert_eq!(access, Access::ReadOnly);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_gives_read_write_access_if_the_key_is_authorized() {
        let owner_key = generate_ed25519_key();
        let owner_did = Did(owner_key.get_did().await.unwrap());
        let mut db = SphereDb::new(&MemoryStorage::default()).await.unwrap();

        let (sphere, authorization, _) = Sphere::try_generate(&owner_did, &mut db).await.unwrap();
        let author = Author {
            key: owner_key,
            authorization: Some(authorization),
        };

        let access = author
            .access_to(&sphere.try_get_identity().await.unwrap(), &db)
            .await
            .unwrap();

        assert_eq!(access, Access::ReadWrite);
    }
}
