use anyhow::anyhow;
use anyhow::Result;
use cid::Cid;
use noosphere_core::data::{AddressIpld, Did, Jwt, MapOperation};
use noosphere_ns::NsRecord;
use noosphere_ns::{server::HttpClient as NameSystemHttpClient, NameSystemClient};
use noosphere_sphere::{
    HasMutableSphereContext, SphereContext, SphereCursor, SpherePetnameRead, SpherePetnameWrite,
};
use noosphere_storage::Storage;
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::str::FromStr;
use std::sync::Arc;
use std::time::Duration;
use strum_macros::Display;
use tokio::{
    sync::{
        mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
        oneshot::Sender,
        Mutex,
    },
    task::JoinHandle,
};
use tokio_stream::{Stream, StreamExt};
use ucan::crypto::KeyMaterial;
use url::Url;

#[derive(Clone)]
pub enum NameSystemConfiguration {
    Remote(Url),
    // TODO: Configuration for self-managed node
    //InProcess(...)
}

#[derive(Display)]
pub enum NameSystemJob<K, S>
where
    K: KeyMaterial + Clone + 'static,
    S: Storage,
{
    /// Resolve all names in the sphere at the latest version
    ResolveAll {
        context: Arc<Mutex<SphereContext<K, S>>>,
    },
    /// Resolve a single name from a given sphere at the latest version
    #[allow(dead_code)]
    ResolveImmediately {
        context: Arc<Mutex<SphereContext<K, S>>>,
        name: String,
        tx: Sender<Option<Cid>>,
    },
    /// Resolve all added names of a given sphere since the given sphere
    /// revision
    ResolveSince {
        context: Arc<Mutex<SphereContext<K, S>>>,
        since: Option<Cid>,
    },
    Publish {
        context: Arc<Mutex<SphereContext<K, S>>>,
        record: Jwt,
    },
}

pub fn start_name_system<K, S>(
    configuration: NameSystemConfiguration,
    local_spheres: Vec<Arc<Mutex<SphereContext<K, S>>>>,
) -> (UnboundedSender<NameSystemJob<K, S>>, JoinHandle<Result<()>>)
where
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

async fn periodic_resolver_task<K, S>(
    tx: UnboundedSender<NameSystemJob<K, S>>,
    local_spheres: Vec<Arc<Mutex<SphereContext<K, S>>>>,
) where
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

pub async fn name_system_task<K, S>(
    configuration: NameSystemConfiguration,
    mut receiver: UnboundedReceiver<NameSystemJob<K, S>>,
) -> Result<()>
where
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    let client: Arc<dyn NameSystemClient> = Arc::new(match configuration {
        NameSystemConfiguration::Remote(url) => NameSystemHttpClient::new(url).await?,
    });

    while let Some(job) = receiver.recv().await {
        let run_job = || async {
            debug!("Running {}", job);
            match job {
                NameSystemJob::Publish { record, .. } => {
                    client.put_record(NsRecord::from_str(&record)?).await?;
                }
                NameSystemJob::ResolveAll { context } => {
                    let name_stream = {
                        let context = context.lock().await;
                        let sphere = context.sphere().await?;
                        let names = sphere.get_names().await?;

                        names.into_stream().await?
                    };

                    resolve_all(client.clone(), context, name_stream).await?;
                }
                NameSystemJob::ResolveSince { context, since } => {
                    let history_stream = {
                        let context = context.lock().await;
                        let sphere = context.sphere().await?;
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

                    let mut names_to_resolve = BTreeMap::<String, AddressIpld>::new();
                    let mut names_to_ignore = BTreeSet::new();

                    for (_, sphere) in reverse_history {
                        let names = sphere.get_names().await?;
                        let changelog = names.try_load_changelog().await?;

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
                        tokio_stream::iter(names_to_resolve.into_iter().map(|item| Ok(item))),
                    )
                    .await?;
                }
                NameSystemJob::ResolveImmediately { context, name, tx } => {
                    // TODO: This is going to be blocked by any pending "resolve all" jobs. We should consider
                    // delaying "resolve all" so that an eager client can get ahead of the queue if desired. Even
                    // better would be some kind of streamed priority queue for resolutions, but that's a more
                    // involved enhancement.
                    let stream = {
                        let context = context.lock().await;
                        let sphere = context.sphere().await?;
                        let names = sphere.get_names().await?;
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

                    resolve_all(client.clone(), context.clone(), stream).await?;

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
async fn resolve_all<H, K, S, N>(
    client: Arc<dyn NameSystemClient>,
    mut context: H,
    stream: N,
) -> Result<()>
where
    H: HasMutableSphereContext<K, S>,
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
    N: Stream<Item = Result<(String, AddressIpld)>>,
{
    tokio::pin!(stream);

    while let Some((name, address)) = stream.try_next().await? {
        let last_known_record = address.last_known_record.clone();

        let next_record =
            match resolve_record(client.clone(), name.clone(), address.identity.clone()).await? {
                Some(token) => {
                    // TODO: Verify that the new value is the most recent value
                    // TODO: Verify the proof chain of the new value
                    Some(token)
                }
                None => {
                    // TODO: Expire recorded value if we don't get an updated
                    // record after some designated TTL
                    continue;
                }
            };

        match &next_record {
            // TODO: What if the resolved value is None?
            Some(record) if last_known_record != next_record => {
                debug!("ADOPTING PETNAME RECORD...");
                context
                    .adopt_petname(&name, &address.identity, record)
                    .await?;
            }
            _ => continue,
        }
    }

    if context.has_unsaved_changes().await? {
        SphereCursor::latest(context).save(None).await?;
    }

    Ok(())
}

/// Attempts to resolve a single name record from the name system
async fn resolve_record(
    client: Arc<dyn NameSystemClient>,
    name: String,
    identity: Did,
) -> Result<Option<Jwt>> {
    debug!("Resolving record '{}' ({})...", name, identity);
    Ok(match client.get_record(&identity).await {
        Ok(Some(record)) => match record.try_to_string() {
            Ok(token) => {
                debug!("Resolved record for '{}' ({}): {}", name, identity, token);
                Some(token.into())
            }
            Err(error) => {
                warn!("Failed to interpret resolved record as JWT: {:?}", error);
                None
            }
        },
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
pub struct OnDemandNameResolver<K, S>(UnboundedSender<NameSystemJob<K, S>>)
where
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static;

impl<K, S> OnDemandNameResolver<K, S>
where
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    #[allow(dead_code)]
    pub async fn resolve(
        &self,
        context: Arc<Mutex<SphereContext<K, S>>>,
        name: &str,
    ) -> Result<Option<Cid>> {
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
