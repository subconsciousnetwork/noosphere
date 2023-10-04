use std::{io::Cursor, sync::Arc};

use anyhow::Result;
use libipld_cbor::DagCborCodec;
use noosphere_core::context::{
    metadata::COUNTERPART, HasMutableSphereContext, SphereContentRead, SphereContentWrite,
    SphereCursor,
};
use noosphere_core::{
    data::{ContentType, Did, Link, MemoIpld},
    view::Timeline,
};
use noosphere_ipfs::{IpfsClient, KuboClient};
use noosphere_storage::{block_deserialize, block_serialize, BlockStore, KeyValueStore, Storage};
use serde::{Deserialize, Serialize};
use tokio::{
    io::AsyncReadExt,
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    task::JoinHandle,
};
use tokio_stream::StreamExt;
use url::Url;

use deterministic_bloom::const_size::BloomFilter;
use iroh_car::{CarHeader, CarWriter};

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
/// sphere that was successfully syndicated to an IPFS node. It records a Bloom
/// filter populated by the CIDs of all blocks that have been syndicated, which
/// gives us a short-cut to determine if a block should be added.
#[derive(Serialize, Deserialize)]
pub struct SyndicationCheckpoint {
    pub revision: Link<MemoIpld>,
    pub syndicated_blocks: BloomFilter<256, 30>,
}

/// Start a Tokio task that waits for [SyndicationJob] messages and then
/// attempts to syndicate to the configured IPFS RPC. Currently only Kubo IPFS
/// backends are supported.
pub fn start_ipfs_syndication<C, S>(
    ipfs_api: Url,
) -> (UnboundedSender<SyndicationJob<C>>, JoinHandle<Result<()>>)
where
    C: HasMutableSphereContext<S> + 'static,
    S: Storage + 'static,
{
    let (tx, rx) = unbounded_channel();

    (tx, tokio::task::spawn(ipfs_syndication_task(ipfs_api, rx)))
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
    let (sphere_revision, ancestor_revision, mut syndicated_blocks, db) = {
        let db = {
            let context = context.sphere_context().await?;
            context.db().clone()
        };

        let counterpart_identity = db.require_key::<_, Did>(COUNTERPART).await?;
        let sphere = context.to_sphere().await?;
        let content = sphere.get_content().await?;

        let counterpart_revision = content.require(&counterpart_identity).await?.clone();

        let (last_syndicated_revision, syndicated_blocks) =
            match context.read(&checkpoint_key).await? {
                Some(mut file) => match file.memo.content_type() {
                    Some(ContentType::Cbor) => {
                        let mut bytes = Vec::new();
                        file.contents.read_to_end(&mut bytes).await?;
                        let SyndicationCheckpoint {
                            revision,
                            syndicated_blocks,
                        } = block_deserialize::<DagCborCodec, _>(&bytes)?;
                        (Some(revision), syndicated_blocks)
                    }
                    _ => (None, BloomFilter::default()),
                },
                None => (None, BloomFilter::default()),
            };

        (
            counterpart_revision,
            last_syndicated_revision,
            syndicated_blocks,
            db,
        )
    };

    let timeline = Timeline::new(&db)
        .slice(&sphere_revision, ancestor_revision.as_ref())
        .to_chronological()
        .await?;

    // For all CIDs since the last historical checkpoint, syndicate a CAR
    // of blocks that are unique to that revision to the backing IPFS
    // implementation
    for cid in timeline {
        // TODO(#175): At each increment, if there are sub-graphs of a
        // sphere that should *not* be syndicated (e.g., other spheres
        // referenced by this sphere that are probably syndicated
        // elsewhere), we should add them to the bloom filter at this spot.

        let stream = db.query_links(&cid, {
            let filter = Arc::new(syndicated_blocks.clone());

            move |cid| {
                let filter = filter.clone();
                // let kubo_client = kubo_client.clone();
                let cid = *cid;

                async move {
                    // The Bloom filter probabilistically tells us if we
                    // have syndicated a block; it is probabilistic because
                    // `contains` may give us false positives. But, all
                    // negatives are guaranteed to not have been added. So,
                    // we can rely on it as a short cut to find unsyndicated
                    // blocks, and for positives we can verify the pin
                    // status with the IPFS node.
                    if !filter.contains(&cid.to_bytes()) {
                        return Ok(true);
                    }

                    Ok(false)
                }
            }
        });

        // TODO(#2): It would be cool to make reading from storage and
        // writing to an HTTP request body concurrent / streamed; this way
        // we could send over CARs of arbitrary size (within the limits of
        // whatever the IPFS receiving implementation can support).
        let mut car = Vec::new();
        let car_header = CarHeader::new_v1(vec![cid.clone().into()]);
        let mut car_writer = CarWriter::new(car_header, &mut car);

        tokio::pin!(stream);

        loop {
            match stream.try_next().await {
                Ok(Some(cid)) => {
                    trace!("Syndication will include block {}", cid);
                    // TODO(#176): We need to build-up a list of blocks that aren't
                    // able to be loaded so that we can be resilient to incomplete
                    // data when syndicating to IPFS
                    syndicated_blocks.insert(&cid.to_bytes());

                    let block = db.require_block(&cid).await?;

                    car_writer.write(cid, block).await?;
                }
                Err(error) => {
                    warn!("Encountered error while streaming links: {:?}", error);
                }
                _ => break,
            }
        }

        match kubo_client.syndicate_blocks(Cursor::new(car)).await {
            Ok(_) => debug!("Syndicated sphere revision {} to IPFS", cid),
            Err(error) => warn!("Failed to syndicate revision {} to IPFS: {:?}", cid, error),
        };
    }

    // At the end, take another lock on the `SphereContext` in order to
    // update the syndication checkpoint for this particular IPFS server
    {
        let mut cursor = SphereCursor::latest(context.clone());
        let (_, bytes) = block_serialize::<DagCborCodec, _>(&SyndicationCheckpoint {
            revision,
            syndicated_blocks,
        })?;

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
