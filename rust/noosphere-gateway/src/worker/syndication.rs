use crate::GatewayManager;
use anyhow::Result;
use cid::Cid;
use libipld_cbor::DagCborCodec;
use noosphere_common::UnsharedStream;
use noosphere_core::context::{
    metadata::COUNTERPART, HasMutableSphereContext, SphereContentRead, SphereContentWrite,
    SphereCursor,
};
use noosphere_core::stream::{memo_body_stream, record_stream_orphans, to_car_stream};
use noosphere_core::{
    data::{ContentType, Did, Link, MemoIpld},
    view::Timeline,
};
use noosphere_ipfs::{IpfsClient, KuboClient};
use noosphere_storage::{block_deserialize, block_serialize, KeyValueStore, Storage};
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{io::Cursor, sync::Arc};
use tokio::sync::Mutex;
use tokio::{
    io::AsyncReadExt,
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    task::JoinHandle,
};
use tokio_stream::StreamExt;
use tokio_util::io::StreamReader;
use url::Url;

/// A [SyndicationJob] is a request to syndicate the blocks of a _counterpart_
/// sphere to the broader IPFS network.
pub struct SyndicationJob<C> {
    /// The revision of the _local_ sphere to discover the _counterpart_ sphere
    /// from; the counterpart sphere's revision will need to be derived using
    /// this checkpoint in local sphere history.
    pub revision: Link<MemoIpld>,
    /// The [SphereContext] that corresponds to the _local_ sphere relative to
    /// the gateway.
    pub context: C,
}

/// A [SyndicationCheckpoint] represents the last spot in the history of a
/// sphere that was successfully syndicated to an IPFS node.
#[derive(Serialize, Deserialize)]
pub struct SyndicationCheckpoint {
    pub last_syndicated_counterpart_version: Option<Link<MemoIpld>>,
    pub syndication_epoch: u64,
}

impl SyndicationCheckpoint {
    pub fn new() -> Result<Self> {
        Ok(Self {
            last_syndicated_counterpart_version: None,
            syndication_epoch: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        })
    }

    pub fn lifetime(&self) -> Result<Duration> {
        Ok(Duration::from_secs(
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)?
                .as_secs()
                - self.syndication_epoch,
        ))
    }

    pub fn is_expired(&self) -> Result<bool> {
        Ok(self.lifetime()? > MAX_SYNDICATION_CHECKPOINT_LIFETIME)
    }
}

// Force full re-syndicate every 90 days
const MAX_SYNDICATION_CHECKPOINT_LIFETIME: Duration = Duration::from_secs(60 * 60 * 24 * 90);

// Periodic syndication check every 5 minutes
const PERIODIC_SYNDICATION_INTERVAL_SECONDS: Duration = Duration::from_secs(5 * 60);

/// Start a Tokio task that waits for [SyndicationJob] messages and then
/// attempts to syndicate to the configured IPFS RPC. Currently only Kubo IPFS
/// backends are supported.
pub fn start_ipfs_syndication<M, C, S>(
    ipfs_api: Url,
    gateway_manager: M,
) -> (UnboundedSender<SyndicationJob<C>>, JoinHandle<Result<()>>)
where
    M: GatewayManager<C, S> + 'static,
    C: HasMutableSphereContext<S> + 'static,
    S: Storage + 'static,
{
    let (tx, rx) = unbounded_channel();

    let task = {
        let tx = tx.clone();
        tokio::task::spawn(async move {
            let (_, syndication_result) = tokio::join!(
                periodic_syndication_task(tx, gateway_manager),
                ipfs_syndication_task(ipfs_api, rx)
            );
            syndication_result?;
            Ok(())
        })
    };

    (tx, task)
}

async fn periodic_syndication_task<M, C, S>(
    tx: UnboundedSender<SyndicationJob<C>>,
    gateway_manager: M,
) where
    M: GatewayManager<C, S> + 'static,
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
{
    loop {
        let mut stream = gateway_manager.experimental_worker_only_iter();
        loop {
            match stream.try_next().await {
                Ok(Some(local_sphere)) => {
                    if let Err(error) = periodic_syndication(&tx, local_sphere).await {
                        error!("Periodic syndication of sphere history failed: {}", error);
                    };
                }
                Ok(None) => {
                    break;
                }
                Err(error) => {
                    error!("Could not iterate on managed spheres: {}", error);
                }
            }
            tokio::time::sleep(Duration::from_secs(5)).await;
        }
        tokio::time::sleep(PERIODIC_SYNDICATION_INTERVAL_SECONDS).await;
    }
}

async fn periodic_syndication<C, S>(
    tx: &UnboundedSender<SyndicationJob<C>>,
    local_sphere: C,
) -> Result<()>
where
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
{
    let latest_version = local_sphere.version().await?;

    if let Err(error) = tx.send(SyndicationJob {
        revision: latest_version,
        context: local_sphere.clone(),
    }) {
        warn!("Failed to request periodic syndication: {}", error);
    };

    Ok(())
}

async fn ipfs_syndication_task<C, S>(
    ipfs_api: Url,
    mut receiver: UnboundedReceiver<SyndicationJob<C>>,
) -> Result<()>
where
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
{
    debug!("Syndicating sphere revisions to IPFS API at {}", ipfs_api);

    let kubo_client = Arc::new(KuboClient::new(&ipfs_api)?);

    while let Some(job) = receiver.recv().await {
        if let Err(error) = process_job(job, kubo_client.clone(), &ipfs_api).await {
            warn!("Error processing IPFS job: {}", error);
        }
    }
    Ok(())
}

#[instrument(skip(job, kubo_client))]
async fn process_job<C, S>(
    job: SyndicationJob<C>,
    kubo_client: Arc<KuboClient>,
    ipfs_api: &Url,
) -> Result<()>
where
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
{
    let SyndicationJob { revision, context } = job;
    debug!("Attempting to syndicate version DAG {revision} to IPFS");
    let kubo_identity = kubo_client.server_identity().await.map_err(|error| {
        anyhow::anyhow!(
            "Failed to identify an IPFS Kubo node at {}: {}",
            ipfs_api,
            error
        )
    })?;
    let checkpoint_key = format!("syndication/kubo/{kubo_identity}");

    debug!("IPFS node identified as {}", kubo_identity);

    // Take a lock on the `SphereContext` and look up the most recent
    // syndication checkpoint for this Kubo node
    let (sphere_revision, mut syndication_checkpoint, db) = {
        let db = {
            let context = context.sphere_context().await?;
            context.db().clone()
        };

        let counterpart_identity = db.require_key::<_, Did>(COUNTERPART).await?;
        let sphere = context.to_sphere().await?;
        let content = sphere.get_content().await?;

        let counterpart_revision = *content.require(&counterpart_identity).await?;

        let syndication_checkpoint = match context.read(&checkpoint_key).await? {
            Some(mut file) => match file.memo.content_type() {
                Some(ContentType::Cbor) => {
                    let mut bytes = Vec::new();
                    file.contents.read_to_end(&mut bytes).await?;
                    let current_checkpoint = match block_deserialize::<DagCborCodec, _>(&bytes) {
                        Ok(checkpoint) => checkpoint,
                        _ => SyndicationCheckpoint::new()?,
                    };

                    if current_checkpoint.is_expired()? {
                        SyndicationCheckpoint::new()?
                    } else {
                        current_checkpoint
                    }
                }
                _ => SyndicationCheckpoint::new()?,
            },
            None => SyndicationCheckpoint::new()?,
        };

        if Some(counterpart_revision) == syndication_checkpoint.last_syndicated_counterpart_version
        {
            warn!("Counterpart version hasn't changed; skipping syndication");
            return Ok(());
        }

        (counterpart_revision, syndication_checkpoint, db)
    };

    let timeline = Timeline::new(&db)
        .slice(
            &sphere_revision,
            syndication_checkpoint
                .last_syndicated_counterpart_version
                .as_ref(),
        )
        .exclude_past()
        .to_chronological()
        .await?;

    // For all CIDs since the last historical checkpoint, syndicate a CAR
    // of blocks that are unique to that revision to the backing IPFS
    // implementation

    for cid in timeline {
        let orphans = Arc::new(Mutex::new(Vec::new()));

        let car_stream = to_car_stream(
            vec![cid.into()],
            record_stream_orphans(orphans.clone(), memo_body_stream(db.clone(), &cid, true)),
        );
        let unshared_stream = UnsharedStream::new(Box::pin(car_stream));
        let car_reader = StreamReader::new(unshared_stream);

        match kubo_client.syndicate_blocks(car_reader).await {
            Ok(_) => {
                let orphans = orphans.lock().await;

                debug!(?orphans, "Imported blocks to IPFS; pinning orphans...",);

                let root = Cid::from(cid);

                match kubo_client
                    .pin_blocks(orphans.iter().filter(|orphan| *orphan != &root))
                    .await
                {
                    Ok(_) => {
                        debug!("Syndicated sphere revision {} to IPFS", cid);
                        syndication_checkpoint.last_syndicated_counterpart_version = Some(cid);
                    }
                    Err(error) => warn!(
                        "Failed to pin orphans for revision {} to IPFS: {:?}",
                        cid, error
                    ),
                }
            }
            Err(error) => warn!("Failed to syndicate revision {} to IPFS: {:?}", cid, error),
        };
    }

    // At the end, take another lock on the `SphereContext` in order to
    // update the syndication checkpoint for this particular IPFS server
    {
        let mut cursor = SphereCursor::latest(context.clone());
        let (_, bytes) = block_serialize::<DagCborCodec, _>(&syndication_checkpoint)?;

        cursor
            .write(
                &checkpoint_key,
                &ContentType::Cbor,
                Cursor::new(bytes),
                None,
            )
            .await?;

        cursor.save(None).await?;
    }
    Ok(())
}

#[cfg(all(test, feature = "test-kubo"))]
mod tests {
    use std::time::Duration;

    use anyhow::Result;
    use noosphere_common::helpers::wait;
    use noosphere_core::{
        authority::Access,
        context::{HasMutableSphereContext, HasSphereContext, SphereContentWrite, COUNTERPART},
        data::ContentType,
        helpers::simulated_sphere_context,
        tracing::initialize_tracing,
    };
    use noosphere_ipfs::{IpfsClient, KuboClient};
    use noosphere_storage::KeyValueStore;
    use tokio::select;
    use url::Url;

    use crate::{
        worker::{start_ipfs_syndication, SyndicationCheckpoint, SyndicationJob},
        SingleTenantGatewayManager,
    };

    #[tokio::test(flavor = "multi_thread")]
    async fn it_syndicates_a_sphere_revision_to_kubo() -> Result<()> {
        initialize_tracing(None);

        let (mut user_sphere_context, _) =
            simulated_sphere_context(Access::ReadWrite, None).await?;

        let (mut gateway_sphere_context, _) = simulated_sphere_context(
            Access::ReadWrite,
            Some(user_sphere_context.lock().await.db().clone()),
        )
        .await?;

        let user_sphere_identity = user_sphere_context.identity().await?;

        gateway_sphere_context
            .lock()
            .await
            .db_mut()
            .set_key(COUNTERPART, &user_sphere_identity)
            .await?;

        let ipfs_url = Url::parse("http://127.0.0.1:5001")?;
        let local_kubo_client = KuboClient::new(&ipfs_url.clone())?;
        let manager = SingleTenantGatewayManager::new(
            gateway_sphere_context.clone(),
            user_sphere_identity.clone(),
        )
        .await?;

        let (syndication_tx, _syndication_join_handle) =
            start_ipfs_syndication::<_, _, _>(ipfs_url, manager);

        user_sphere_context
            .write("foo", &ContentType::Text, b"bar".as_ref(), None)
            .await?;

        user_sphere_context.save(None).await?;

        user_sphere_context
            .write("baz", &ContentType::Text, b"bar".as_ref(), None)
            .await?;

        let version = user_sphere_context.save(None).await?;

        gateway_sphere_context
            .link_raw(&user_sphere_identity, &version)
            .await?;
        gateway_sphere_context.save(None).await?;

        debug!("Sending syndication job...");
        syndication_tx.send(SyndicationJob {
            revision: version.clone(),
            context: gateway_sphere_context.clone(),
        })?;

        debug!("Giving syndication a moment to complete...");

        wait(1).await;

        debug!("Looking for blocks...");

        for _ in 0..30 {
            debug!("Sending request to Kubo...");

            select! {
                maybe_block = local_kubo_client.get_block(&version) => {
                    if maybe_block?.is_some() {
                        debug!("Found block!");
                        return Ok(());
                    }
                }
                _ = tokio::time::sleep(Duration::from_secs(1)) => ()
            }

            debug!("No block, retrying in one second...");

            wait(1).await;
        }

        unreachable!("Syndicated block should be pinned")
    }

    #[tokio::test]
    async fn it_advances_syndication_checkpoint_lifetime_with_clock_time() -> Result<()> {
        let checkpoint = SyndicationCheckpoint::new()?;

        let lifetime = checkpoint.lifetime()?;

        assert!(lifetime.as_secs() < 1);

        wait(1).await;

        let lifetime = checkpoint.lifetime()?;

        assert!(lifetime.as_secs() >= 1);
        assert!(lifetime.as_secs() < 2);

        wait(2).await;

        let lifetime = checkpoint.lifetime()?;

        assert!(lifetime.as_secs() >= 3);
        assert!(lifetime.as_secs() < 4);

        Ok(())
    }
}
