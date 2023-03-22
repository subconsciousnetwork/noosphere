use anyhow::anyhow;
use anyhow::Result;
use cid::Cid;
use noosphere_core::data::{Did, IdentityIpld, Jwt, LinkRecord, MapOperation};
use noosphere_ipfs::{IpfsStore, KuboClient};
use noosphere_ns::NsRecord;
use noosphere_ns::{server::HttpClient as NameSystemHttpClient, NameSystemClient};
use noosphere_sphere::{
    HasMutableSphereContext, SphereCursor, SpherePetnameRead, SpherePetnameWrite,
};
use noosphere_storage::{BlockStoreRetry, MemoryStore, Storage, UcanStore};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use strum_macros::Display;
use tokio::{
    sync::{
        mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
        oneshot::Sender,
    },
    task::JoinHandle,
};
use tokio_stream::{Stream, StreamExt};
use ucan::crypto::KeyMaterial;
use url::Url;

pub struct NameSystemConfiguration {
    pub connection_type: NameSystemConnectionType,
    pub ipfs_api: Url,
}

#[derive(Clone)]
pub enum NameSystemConnectionType {
    Remote(Url),
    // TODO(#255): Configuration for self-managed node
    //InProcess(...)
}

#[derive(Display)]
pub enum NameSystemJob<C> {
    /// Resolve all names in the sphere at the latest version
    ResolveAll {
        context: C,
    },
    /// Resolve a single name from a given sphere at the latest version
    #[allow(dead_code)]
    ResolveImmediately {
        context: C,
        name: String,
        tx: Sender<Option<Cid>>,
    },
    /// Resolve all added names of a given sphere since the given sphere
    /// revision
    ResolveSince {
        context: C,
        since: Option<Cid>,
    },
    Publish {
        context: C,
        record: Jwt,
    },
}

pub fn start_name_system<C, K, S>(
    configuration: NameSystemConfiguration,
    local_spheres: Vec<C>,
) -> (UnboundedSender<NameSystemJob<C>>, JoinHandle<Result<()>>)
where
    C: HasMutableSphereContext<K, S> + 'static,
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    let (tx, rx) = unbounded_channel();

    let task = {
        let tx = tx.clone();
        tokio::task::spawn(async move {
            let _ = tokio::join!(
                name_system_task(configuration, rx),
                periodic_resolver_task(tx, local_spheres)
            );
            Ok(())
        })
    };

    (tx, task)
}

async fn periodic_resolver_task<C, K, S>(
    tx: UnboundedSender<NameSystemJob<C>>,
    local_spheres: Vec<C>,
) where
    C: HasMutableSphereContext<K, S>,
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    for sphere in local_spheres.iter().cycle() {
        match tx.send(NameSystemJob::ResolveAll {
            context: sphere.clone(),
        }) {
            Ok(_) => (),
            Err(error) => {
                warn!("Failed to request updated name resolutions: {}", error);
            }
        }

        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}

async fn name_system_task<C, K, S>(
    configuration: NameSystemConfiguration,
    mut receiver: UnboundedReceiver<NameSystemJob<C>>,
) -> Result<()>
where
    C: HasMutableSphereContext<K, S>,
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    let client: Arc<dyn NameSystemClient> = Arc::new(match configuration.connection_type {
        NameSystemConnectionType::Remote(url) => NameSystemHttpClient::new(url).await?,
    });
    let kubo_client = KuboClient::new(&configuration.ipfs_api)?;

    while let Some(job) = receiver.recv().await {
        let run_job = || async {
            debug!("Running {}", job);
            match job {
                NameSystemJob::Publish { record, .. } => {
                    client.put_record(NsRecord::from_str(&record)?).await?;
                }
                NameSystemJob::ResolveAll { context } => {
                    let name_stream = {
                        let sphere = context.to_sphere().await?;
                        let names = sphere.get_address_book().await?.get_identities().await?;

                        names.into_stream().await?
                    };

                    resolve_all(client.clone(), context, name_stream, kubo_client.clone()).await?;
                }
                NameSystemJob::ResolveSince { context, since } => {
                    let history_stream = {
                        let sphere = context.to_sphere().await?;
                        sphere.into_history_stream(since.as_ref())
                    };

                    tokio::pin!(history_stream);

                    let reverse_history = history_stream
                        .fold(VecDeque::new(), |mut all, step| {
                            if let Ok(entry) = step {
                                all.push_front(entry);
                            }
                            all
                        })
                        .await;

                    let mut names_to_resolve = BTreeMap::<String, IdentityIpld>::new();
                    let mut names_to_ignore = BTreeSet::new();

                    for (_, sphere) in reverse_history {
                        let names = sphere.get_address_book().await?.get_identities().await?;
                        let changelog = names.load_changelog().await?;

                        for operation in changelog.changes.iter() {
                            match operation {
                                MapOperation::Add { key, value } => {
                                    // Walking backwards through history, we will
                                    // ignore any name changes where the name has
                                    // either been updated or removed in the future
                                    if !names_to_ignore.contains(key)
                                        && !names_to_resolve.contains_key(key)
                                    {
                                        names_to_resolve.insert(key.clone(), value.clone());
                                    }
                                }
                                MapOperation::Remove { key } => {
                                    names_to_ignore.insert(key.clone());
                                }
                            };
                        }
                    }

                    resolve_all(
                        client.clone(),
                        context,
                        tokio_stream::iter(names_to_resolve.into_iter().map(Ok)),
                        kubo_client.clone(),
                    )
                    .await?;
                }
                NameSystemJob::ResolveImmediately { context, name, tx } => {
                    // TODO(#256): This is going to be blocked by any pending
                    // "resolve all" jobs. We should consider delaying "resolve
                    // all" so that an eager client can get ahead of the queue
                    // if desired. Even better would be some kind of streamed
                    // priority queue for resolutions, but that's a more
                    // involved enhancement.
                    let stream = {
                        let sphere = context.to_sphere().await?;
                        let names = sphere.get_address_book().await?.get_identities().await?;
                        let address = names.get(&name).await?;

                        match address {
                            Some(address) => {
                                tokio_stream::once(Ok((name.clone(), address.clone())))
                            }
                            None => {
                                let _ = tx.send(None);
                                return Ok(()) as Result<()>;
                            }
                        }
                    };

                    resolve_all(client.clone(), context.clone(), stream, kubo_client.clone())
                        .await?;

                    let cid = context.resolve_petname(&name).await?;

                    let _ = tx.send(cid);
                }
            };
            Ok(())
        };

        match run_job().await {
            Err(error) => error!("NNS job failed: {}", error),
            _ => debug!("NNS job completed successfully"),
        }
    }

    Ok(())
}

/// Consumes a stream of name / address tuples, resolving them one at a time
/// and updating the provided [SphereContext] with the latest resolved values
async fn resolve_all<C, K, S, N>(
    client: Arc<dyn NameSystemClient>,
    mut context: C,
    stream: N,
    ipfs_client: KuboClient,
) -> Result<()>
where
    C: HasMutableSphereContext<K, S>,
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
    N: Stream<Item = Result<(String, IdentityIpld)>>,
{
    tokio::pin!(stream);

    let db = context.sphere_context().await?.db().clone();
    let ipfs_store = {
        let inner = MemoryStore::default();
        let inner = IpfsStore::new(inner, Some(ipfs_client));
        let inner = BlockStoreRetry::new(inner, 5u32, Duration::new(1, 0));
        UcanStore(inner)
    };

    while let Some((name, identity)) = stream.try_next().await? {
        let last_known_record = identity.link_record(&db).await;

        let next_record =
            match fetch_record(client.clone(), name.clone(), identity.did.clone()).await? {
                Some(record) => {
                    if let Err(error) = record.validate(&ipfs_store, None).await {
                        error!("Failed record validation: {}", error);
                        continue;
                    }

                    // TODO(#258): Verify that the new value is the most recent value
                    Some(LinkRecord::from(Jwt(record.try_to_string()?)))
                }
                None => {
                    // TODO(#259): Expire recorded value if we don't get an updated
                    // record after some designated TTL
                    continue;
                }
            };

        match &next_record {
            // TODO(#260): What if the resolved value is None?
            Some(record) if last_known_record != next_record => {
                debug!(
                    "Gateway adopting petname record for '{}' ({}): {}",
                    name, identity.did, &record
                );
                context.adopt_petname(&name, record).await?;
            }
            _ => continue,
        }
    }

    if context.has_unsaved_changes().await? {
        SphereCursor::latest(context).save(None).await?;
    }

    Ok(())
}

/// Attempts to fetch a single name record from the name system.
async fn fetch_record(
    client: Arc<dyn NameSystemClient>,
    name: String,
    identity: Did,
) -> Result<Option<NsRecord>> {
    debug!("Resolving record '{}' ({})...", name, identity);
    Ok(match client.get_record(&identity).await {
        Ok(Some(record)) => {
            debug!(
                "Resolved record for '{}' ({}): {}",
                name,
                identity,
                record.to_string()
            );
            Some(record)
        }
        Ok(None) => {
            warn!("No record found for {} ({})", name, identity);
            None
        }
        Err(error) => {
            warn!("Failed to resolve '{}' ({}): {:?}", name, identity, error);
            None
        }
    })
}

#[allow(dead_code)]
pub struct OnDemandNameResolver<H>(UnboundedSender<NameSystemJob<H>>);

impl<H> OnDemandNameResolver<H> {
    #[allow(dead_code)]
    pub async fn resolve(&self, context: H, name: &str) -> Result<Option<Cid>> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.0
            .send(NameSystemJob::ResolveImmediately {
                context,
                name: name.to_string(),
                tx,
            })
            .map_err(|error| anyhow!(error.to_string()))?;
        Ok(rx.await?)
    }
}
