use crate::jobs::GatewayJob;
use anyhow::Result;
use cid::Cid;
use libipld_cbor::DagCborCodec;
use noosphere_common::UnsharedStream;
use noosphere_core::context::{
    metadata::COUNTERPART, HasMutableSphereContext, SphereContentRead, SphereContentWrite,
    SphereCursor,
};
use noosphere_core::data::LinkRecord;
use noosphere_core::stream::{memo_body_stream, record_stream_orphans, to_car_stream};
use noosphere_core::{
    data::{ContentType, Did, Link, MemoIpld},
    view::Timeline,
};
use noosphere_ipfs::IpfsClient;
use noosphere_storage::{block_deserialize, block_serialize, KeyValueStore, Storage};
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use std::{io::Cursor, sync::Arc};
use tokio::io::AsyncReadExt;
use tokio::sync::Mutex;
use tokio_util::io::StreamReader;

/// A [SyndicationCheckpoint] represents the last spot in the history of a
/// sphere that was successfully syndicated to an IPFS node.
#[derive(Serialize, Deserialize)]
struct SyndicationCheckpoint {
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

/// Syndicate content to IPFS for given `context` since `revision`,
/// optionally publishing a provided [LinkRecord] on success.
#[instrument(skip(context, ipfs_client, name_publish_on_success))]
pub async fn syndicate_to_ipfs<C, S, I>(
    context: C,
    revision: Option<Link<MemoIpld>>,
    ipfs_client: I,
    name_publish_on_success: Option<LinkRecord>,
) -> Result<Option<GatewayJob>>
where
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
    I: IpfsClient + 'static,
{
    let version_str = revision.map_or_else(|| "latest".into(), |link| link.cid.to_string());
    debug!("Attempting to syndicate version DAG {version_str} to IPFS");
    let kubo_identity = ipfs_client
        .server_identity()
        .await
        .map_err(|error| anyhow::anyhow!("IPFS client could not identify itself: {}", error))?;
    let checkpoint_key = format!("syndication/kubo/{kubo_identity}");

    debug!("IPFS node identified as {}", kubo_identity);

    // Take a lock on the `SphereContext` and look up the most recent
    // syndication checkpoint for this Kubo node
    let (sphere_revision, mut syndication_checkpoint, db, counterpart_identity) = {
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
            return Ok(None);
        }

        (
            counterpart_revision,
            syndication_checkpoint,
            db,
            counterpart_identity,
        )
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

        match ipfs_client.syndicate_blocks(car_reader).await {
            Ok(_) => {
                let orphans = orphans.lock().await;

                debug!(?orphans, "Imported blocks to IPFS; pinning orphans...",);

                let root = Cid::from(cid);

                match ipfs_client
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

    Ok(
        name_publish_on_success.map(|record| GatewayJob::NameSystemPublish {
            identity: counterpart_identity,
            record,
        }),
    )
}

#[cfg(all(test, feature = "test-kubo"))]
mod tests {
    use super::*;
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
    use std::time::Duration;
    use tokio::select;
    use url::Url;

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
        syndicate_to_ipfs(
            gateway_sphere_context.clone(),
            Some(version.clone()),
            local_kubo_client.clone(),
            None,
        )
        .await?;

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
