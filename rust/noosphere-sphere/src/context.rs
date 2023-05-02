use std::sync::Arc;

use anyhow::{anyhow, Result};
use cid::Cid;
use futures_util::TryStreamExt;
use libipld_cbor::DagCborCodec;
use noosphere_api::client::Client;

use noosphere_core::{
    authority::{Access, Author, SUPPORTED_KEYS},
    data::{ContentType, Did, MemoIpld, SphereIpld},
    view::{Sphere, SphereMutation},
};
use noosphere_storage::{BlockStore, KeyValueStore, SphereDb, Storage};
use tokio::sync::OnceCell;
use ucan::crypto::{did::DidParser, KeyMaterial};
use url::Url;

use crate::metadata::GATEWAY_URL;

#[cfg(doc)]
use crate::has::HasSphereContext;

/// A [SphereContext] is an accessor construct over locally replicated sphere
/// data. It embodies both the storage layer that contains the sphere's data
/// as the information needed to verify a user's intended level of access to
/// it (e.g., local key material and [ucan::Ucan]-based authorization).
/// Additionally, the [SphereContext] maintains a reference to an API [Client]
/// that may be initialized as the network becomes available.
///
/// All interactions that pertain to a sphere, including reading or writing
/// its contents and syncing with a gateway, flow through the [SphereContext].
pub struct SphereContext<K, S>
where
    K: KeyMaterial + Clone + 'static,
    S: Storage,
{
    sphere_identity: Did,
    origin_sphere_identity: Did,
    author: Author<K>,
    access: OnceCell<Access>,
    db: SphereDb<S>,
    did_parser: DidParser,
    client: OnceCell<Arc<Client<K, SphereDb<S>>>>,
    mutation: SphereMutation,
}

impl<K, S> Clone for SphereContext<K, S>
where
    K: KeyMaterial + Clone + 'static,
    S: Storage,
{
    fn clone(&self) -> Self {
        Self {
            sphere_identity: self.sphere_identity.clone(),
            origin_sphere_identity: self.origin_sphere_identity.clone(),
            author: self.author.clone(),
            access: OnceCell::new(),
            db: self.db.clone(),
            did_parser: DidParser::new(SUPPORTED_KEYS),
            client: self.client.clone(),
            mutation: SphereMutation::new(self.mutation.author()),
        }
    }
}

impl<K, S> SphereContext<K, S>
where
    K: KeyMaterial + Clone + 'static,
    S: Storage,
{
    pub async fn new(
        sphere_identity: Did,
        author: Author<K>,
        db: SphereDb<S>,
        origin_sphere_identity: Option<Did>,
    ) -> Result<Self> {
        let author_did = author.identity().await?;
        let origin_sphere_identity =
            origin_sphere_identity.unwrap_or_else(|| sphere_identity.clone());

        Ok(SphereContext {
            sphere_identity,
            origin_sphere_identity,
            access: OnceCell::new(),
            author,
            db,
            did_parser: DidParser::new(SUPPORTED_KEYS),
            client: OnceCell::new(),
            mutation: SphereMutation::new(&author_did),
        })
    }

    /// The same as [SphereContext::traverse_by_petname], but accepts a linear
    /// sequence of petnames and attempts to recursively traverse through
    /// spheres using that sequence. The sequence is traversed from back to
    /// front. So, if the sequence is "gold", "cat", "bob", it will traverse to
    /// bob, then to bob's cat, then to bob's cat's gold.
    pub async fn traverse_by_petnames(
        &self,
        petname_path: &[String],
    ) -> Result<Option<SphereContext<K, S>>> {
        let mut sphere_context: Option<Self> = None;
        let mut path = Vec::from(petname_path);

        while let Some(petname) = path.pop() {
            let next_sphere_context = match sphere_context {
                None => self.traverse_by_petname(&petname).await?,
                Some(sphere_context) => sphere_context.traverse_by_petname(&petname).await?,
            };
            sphere_context = match next_sphere_context {
                any @ Some(_) => any,
                None => return Ok(None),
            };
        }

        Ok(sphere_context)
    }

    /// Given a petname that has been assigned to a sphere identity within this
    /// sphere's address book, produce a [SphereContext] backed by the same
    /// credentials and storage primitives as this one, but that accesses the
    /// sphere referred to by the provided [Did]. If the local data for the
    /// sphere being traversed to is not available, an attempt will be made to
    /// replicate the data from a Noosphere Gateway.
    #[instrument(level = "debug", fields(origin = self.sphere_identity.as_str()), skip(self))]
    pub async fn traverse_by_petname(&self, petname: &str) -> Result<Option<SphereContext<K, S>>> {
        // Resolve petname to sphere version via address book entry

        let identity = match self
            .sphere()
            .await?
            .get_address_book()
            .await?
            .get_identities()
            .await?
            .get(&petname.to_string())
            .await?
        {
            Some(address) => address.clone(),
            None => {
                warn!("\"{petname}\" is not assigned to an identity");
                return Ok(None);
            }
        };

        debug!("Petname assigned to {:?}", identity);

        let resolved_version = match identity.link_record(self.db()).await {
            Some(link_record) => link_record.dereference().await,
            None => None,
        };

        let resolved_version = match resolved_version {
            Some(cid) => cid,
            None => {
                return Err(anyhow!(
                    "No version has been resolved for \"{petname}\" ({})",
                    identity.did
                ));
            }
        };

        debug!("Resolved version is {}", resolved_version);

        // Check for version in local sphere DB

        let maybe_has_resolved_version = match self.db().get_version(&identity.did).await? {
            Some(local_version) => {
                debug!("Local version: {}", local_version);
                local_version == resolved_version
            }
            None => {
                debug!("No local version");
                false
            }
        };

        // If version available, check for memo and body blocks

        let should_replicate_from_gateway = if maybe_has_resolved_version {
            match self
                .db()
                .load::<DagCborCodec, MemoIpld>(&resolved_version)
                .await
            {
                Ok(memo) => {
                    if memo.content_type() != Some(ContentType::Sphere) {
                        return Err(anyhow!(
                            "Resolved content for \"{petname}\" ({}) does not refer to a sphere",
                            identity.did
                        ));
                    }

                    debug!("Checking to see if we can get the sphere body...");

                    match self.db().load::<DagCborCodec, SphereIpld>(&memo.body).await {
                        Ok(_) => false,
                        Err(error) => {
                            warn!("{error}");
                            true
                        }
                    }
                }
                Err(error) => {
                    warn!("{error}");
                    true
                }
            }
        } else {
            true
        };

        // If no version available or memo/body missing, replicate from gateway
        let mut db = self.db.clone();

        if should_replicate_from_gateway {
            debug!("Attempting to replicate from gateway...");
            let client = self.client().await?;
            let stream = client.replicate(&resolved_version).await?;

            tokio::pin!(stream);

            while let Some((cid, block)) = stream.try_next().await? {
                db.put_block(&cid, &block).await?;
            }

            debug!("Setting local version to resolved version");

            db.set_version(&identity.did, &resolved_version).await?;
        }

        // Initialize a `SphereContext` with the same author and sphere DB as
        // this one, but referring to the resolved sphere DID, and return it

        Ok(Some(
            SphereContext::new(
                identity.did.clone(),
                self.author.clone(),
                self.db.clone(),
                Some(self.sphere_identity.clone()),
            )
            .await?,
        ))
    }

    /// Given a [Did] of a sphere, produce a [SphereContext] backed by the same credentials and
    /// storage primitives as this one, but that accesses the sphere referred to by the provided
    /// [Did].
    pub async fn traverse_by_identity(
        &self,
        _sphere_identity: &Did,
    ) -> Result<SphereContext<K, S>> {
        unimplemented!()
    }

    /// Resolve the most recent version in the local history of the sphere
    pub async fn head(&self) -> Result<Cid> {
        let sphere_identity = self.identity();
        self.db
            .get_version(&self.sphere_identity)
            .await?
            .ok_or_else(|| anyhow!("No version found for {}", sphere_identity))
    }

    /// The identity of the sphere
    pub fn identity(&self) -> &Did {
        &self.sphere_identity
    }

    /// The [Author] who is currently accessing the sphere
    pub fn author(&self) -> &Author<K> {
        &self.author
    }

    /// The [Access] level that the configured [Author] has relative to the
    /// sphere that this [SphereContext] refers to.
    pub async fn access(&self) -> Result<Access> {
        let access = self
            .access
            .get_or_try_init(|| async {
                self.author.access_to(&self.sphere_identity, &self.db).await
            })
            .await?;
        Ok(access.clone())
    }

    /// Get a mutable reference to the [DidParser] used in this [SphereContext]
    pub fn did_parser_mut(&mut self) -> &mut DidParser {
        &mut self.did_parser
    }

    /// Sets or unsets the gateway URL that points to the gateway API that the
    /// sphere will use when it is syncing.
    pub async fn configure_gateway_url(&mut self, url: Option<&Url>) -> Result<()> {
        self.client = OnceCell::new();

        match url {
            Some(url) => {
                self.db.set_key(GATEWAY_URL, url.to_string()).await?;
            }
            None => {
                self.db.unset_key(GATEWAY_URL).await?;
            }
        }

        Ok(())
    }

    /// Get the [SphereDb] instance that manages the current sphere's block
    /// space and persisted configuration.
    pub fn db(&self) -> &SphereDb<S> {
        &self.db
    }

    /// Get a mutable reference to the [SphereDb] instance that manages the
    /// current sphere's block space and persisted configuration.
    pub fn db_mut(&mut self) -> &mut SphereDb<S> {
        &mut self.db
    }

    pub fn mutation(&self) -> &SphereMutation {
        &self.mutation
    }

    pub fn mutation_mut(&mut self) -> &mut SphereMutation {
        &mut self.mutation
    }

    /// Get a [Sphere] view over the current sphere's latest revision. This view
    /// offers lower-level access than [HasSphereContext], but includes affordances to
    /// help tranversing and manipulating IPLD structures that are more
    /// convenient than working directly with raw data.
    pub async fn sphere(&self) -> Result<Sphere<SphereDb<S>>> {
        Ok(Sphere::at(
            &self.db.require_version(self.identity()).await?,
            self.db(),
        ))
    }

    /// Get a [Client] that will interact with a configured gateway (if a URL
    /// for one has been configured). This will initialize a [Client] if one is
    /// not already intialized, and will fail if the [Client] is unable to
    /// verify the identity of the gateway or otherwise connect to it.
    pub async fn client(&self) -> Result<Arc<Client<K, SphereDb<S>>>> {
        let client = self
            .client
            .get_or_try_init::<anyhow::Error, _, _>(|| async {
                let gateway_url: Url = self.db.require_key(GATEWAY_URL).await?;

                Ok(Arc::new(
                    Client::identify(
                        &self.origin_sphere_identity,
                        &gateway_url,
                        &self.author,
                        // TODO: Kill `DidParser` with fire
                        &mut DidParser::new(SUPPORTED_KEYS),
                        self.db.clone(),
                    )
                    .await?,
                ))
            })
            .await?;

        Ok(client.clone())
    }

    // Reset access so that it is re-evaluated the next time it is measured
    // self.access.take();
    pub(crate) fn reset_access(&mut self) {
        self.access.take();
    }
}

#[cfg(test)]
pub mod tests {
    use anyhow::Result;
    use std::sync::Arc;

    use noosphere_core::{
        authority::{SphereAction, SphereReference},
        data::{ContentType, Jwt},
        tracing::initialize_tracing,
    };
    use noosphere_storage::{MemoryStorage, TrackingStorage};
    use serde_json::json;
    use tokio::{io::AsyncReadExt, sync::Mutex};
    use ucan::{
        builder::UcanBuilder,
        capability::{Capability, Resource, With},
    };
    use ucan_key_support::ed25519::Ed25519KeyMaterial;
    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    use crate::{
        helpers::{simulated_sphere_context, SimulationAccess},
        HasMutableSphereContext, HasSphereContext, SphereContentRead, SphereContentWrite,
        SphereContext, SpherePetnameWrite,
    };

    async fn make_sphere_context_with_peer_chain(
        peer_chain: &[String],
    ) -> Result<Arc<Mutex<SphereContext<Ed25519KeyMaterial, TrackingStorage<MemoryStorage>>>>> {
        let origin_sphere_context = simulated_sphere_context(SimulationAccess::ReadWrite, None)
            .await
            .unwrap();

        let mut db = origin_sphere_context
            .sphere_context()
            .await
            .unwrap()
            .db()
            .clone();

        let mut contexts = vec![origin_sphere_context.clone()];

        for name in peer_chain.iter() {
            let mut sphere_context =
                simulated_sphere_context(SimulationAccess::ReadWrite, Some(db.clone()))
                    .await
                    .unwrap();

            sphere_context
                .write(
                    "my-name",
                    &ContentType::Subtext.to_string(),
                    name.as_bytes(),
                    None,
                )
                .await
                .unwrap();
            sphere_context.save(None).await.unwrap();

            contexts.push(sphere_context);
        }

        let mut next_sphere_context: Option<
            Arc<Mutex<SphereContext<Ed25519KeyMaterial, TrackingStorage<MemoryStorage>>>>,
        > = None;

        for mut sphere_context in contexts.into_iter().rev() {
            if let Some(next_sphere_context) = next_sphere_context {
                let version = next_sphere_context.version().await.unwrap();

                let next_author = next_sphere_context
                    .sphere_context()
                    .await
                    .unwrap()
                    .author()
                    .clone();
                let next_identity = next_sphere_context.identity().await.unwrap();

                let link_record = Jwt(UcanBuilder::default()
                    .issued_by(&next_author.key)
                    .for_audience(&next_identity)
                    .witnessed_by(
                        &next_author
                            .authorization
                            .as_ref()
                            .unwrap()
                            .resolve_ucan(&db)
                            .await
                            .unwrap(),
                    )
                    .claiming_capability(&Capability {
                        with: With::Resource {
                            kind: Resource::Scoped(SphereReference {
                                did: next_identity.into(),
                            }),
                        },
                        can: SphereAction::Publish,
                    })
                    .with_lifetime(120)
                    .with_fact(json!({
                    "link": version.to_string()
                    }))
                    .build()
                    .unwrap()
                    .sign()
                    .await
                    .unwrap()
                    .encode()
                    .unwrap());

                let mut name = String::new();
                let mut file = next_sphere_context.read("my-name").await.unwrap().unwrap();
                file.contents.read_to_string(&mut name).await.unwrap();

                debug!("Adopting {name}");

                sphere_context
                    .adopt_petname(&name, &link_record)
                    .await
                    .unwrap();

                db.set_version(
                    &sphere_context.identity().await.unwrap(),
                    &sphere_context.save(None).await.unwrap(),
                )
                .await
                .unwrap();
            }

            next_sphere_context = Some(sphere_context);
        }

        Ok(origin_sphere_context)
    }

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_traverse_a_sequence_of_petnames() {
        initialize_tracing(None);

        let name_seqeuence: Vec<String> = vec!["a".into(), "b".into(), "c".into()];
        let origin_sphere_context = make_sphere_context_with_peer_chain(&name_seqeuence)
            .await
            .unwrap();

        let target_sphere_context = Arc::new(
            origin_sphere_context
                .sphere_context()
                .await
                .unwrap()
                .traverse_by_petnames(&name_seqeuence.into_iter().rev().collect::<Vec<String>>())
                .await
                .unwrap()
                .unwrap(),
        );

        let mut name = String::new();
        let mut file = target_sphere_context
            .read("my-name")
            .await
            .unwrap()
            .unwrap();
        file.contents.read_to_string(&mut name).await.unwrap();

        assert_eq!(name.as_str(), "c");
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_can_traverse_a_sequence_of_petnames_one_at_a_time() {
        initialize_tracing(None);

        let name_seqeuence: Vec<String> = vec!["a".into(), "b".into(), "c".into()];
        let origin_sphere_context = make_sphere_context_with_peer_chain(&name_seqeuence)
            .await
            .unwrap();

        let mut target_sphere_context = origin_sphere_context;

        for name in name_seqeuence.iter() {
            target_sphere_context = Arc::new(Mutex::new(
                target_sphere_context
                    .sphere_context()
                    .await
                    .unwrap()
                    .traverse_by_petnames(&[name.clone()])
                    .await
                    .unwrap()
                    .unwrap(),
            ));
        }

        let mut name = String::new();
        let mut file = target_sphere_context
            .read("my-name")
            .await
            .unwrap()
            .unwrap();
        file.contents.read_to_string(&mut name).await.unwrap();

        assert_eq!(name.as_str(), "c");
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_resolves_none_when_a_petname_is_missing_from_the_sequence() {
        initialize_tracing(None);

        let name_seqeuence: Vec<String> = vec!["b".into(), "c".into()];
        let origin_sphere_context = make_sphere_context_with_peer_chain(&name_seqeuence)
            .await
            .unwrap();

        let traversed_sequence: Vec<String> = vec!["a".into(), "b".into(), "c".into()];

        let target_sphere_context = origin_sphere_context
            .sphere_context()
            .await
            .unwrap()
            .traverse_by_petnames(
                &traversed_sequence
                    .into_iter()
                    .rev()
                    .collect::<Vec<String>>(),
            )
            .await
            .unwrap();

        assert!(target_sphere_context.is_none());
    }
}
