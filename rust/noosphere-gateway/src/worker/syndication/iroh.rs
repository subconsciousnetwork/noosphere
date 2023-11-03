use std::io::Cursor;
use std::path::{Path, PathBuf};

use anyhow::Result;
use iroh::bytes::util::runtime::Handle;
use iroh::node::{Node, DEFAULT_BIND_ADDR};
use iroh::rpc_protocol::{DocTicket, ProviderRequest, ProviderResponse};
use iroh::sync::AuthorId;
use iroh::util::fs::load_secret_key;
use iroh::util::path::IrohPaths;
use libipld_cbor::DagCborCodec;
use noosphere_core::context::{
    metadata::COUNTERPART, HasMutableSphereContext, SphereContentRead, SphereContentWrite,
    SphereCursor,
};
use noosphere_core::stream::memo_body_stream;
use noosphere_core::{
    data::{ContentType, Did, Link, MemoIpld},
    view::Timeline,
};
use noosphere_storage::{block_deserialize, block_serialize, KeyValueStore, Storage};
use quic_rpc::transport::flume::FlumeConnection;
use tokio::{
    io::AsyncReadExt,
    sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
    task::JoinHandle,
};
use tokio_stream::StreamExt;

use crate::worker::SyndicationCheckpoint;

/// A [SyndicationJob] is a request to syndicate the blocks of a _counterpart_
/// sphere to the broader IPFS network.
pub struct SyndicationJobIroh<C> {
    pub revision: Link<MemoIpld>,
    pub context: C,
}

pub fn start_iroh_syndication<C, S>(
    sphere_path: impl AsRef<Path>,
    iroh_ticket: DocTicket,
) -> (
    UnboundedSender<SyndicationJobIroh<C>>,
    JoinHandle<Result<()>>,
)
where
    C: HasMutableSphereContext<S> + 'static,
    S: Storage + 'static,
{
    let (tx, rx) = unbounded_channel();

    (
        tx,
        tokio::task::spawn(iroh_syndication_task(
            sphere_path.as_ref().to_path_buf(),
            iroh_ticket,
            rx,
        )),
    )
}

async fn iroh_syndication_task<C, S>(
    sphere_path: PathBuf,
    ticket: DocTicket,
    mut receiver: UnboundedReceiver<SyndicationJobIroh<C>>,
) -> Result<()>
where
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
{
    debug!("Syndicating sphere revisions to Iroh");

    let iroh_client = Iroh::from_ticket(sphere_path, ticket).await?;

    while let Some(job) = receiver.recv().await {
        if let Err(error) = process_job(job, iroh_client.clone()).await {
            warn!("Error processing IPFS job: {}", error);
        }
    }
    Ok(())
}

#[derive(Clone)]
struct Iroh {
    node: Node<iroh::baomap::flat::Store, iroh::sync::store::fs::Store>,
    #[allow(dead_code)]
    client: iroh::client::Iroh<FlumeConnection<ProviderResponse, ProviderRequest>>,
    doc: iroh::client::Doc<FlumeConnection<ProviderResponse, ProviderRequest>>,
    author: AuthorId,
}

impl Iroh {
    async fn from_ticket(sphere_path: PathBuf, ticket: DocTicket) -> Result<Self> {
        let root = sphere_path.join("storage").join("iroh");
        debug!("Starting iroh at {}", root.display());

        tokio::fs::create_dir_all(&root).await?;

        let rt = Handle::from_current(1)?;

        let peers_data_path = IrohPaths::PeerData.with_root(&root);
        let docs_path = IrohPaths::DocsDatabase.with_root(&root);
        let doc_store = iroh::sync::store::fs::Store::new(&docs_path)?;

        // Optimization: load iroh-bytes store if the block store is an iroh store

        let complete_path = IrohPaths::BaoFlatStoreComplete.with_root(&root);
        let partial_path = IrohPaths::BaoFlatStorePartial.with_root(&root);
        let meta_path = IrohPaths::BaoFlatStoreMeta.with_root(&root);

        tokio::fs::create_dir_all(&complete_path).await?;
        tokio::fs::create_dir_all(&partial_path).await?;
        tokio::fs::create_dir_all(&meta_path).await?;

        let bao_store =
            iroh::baomap::flat::Store::load(&complete_path, &partial_path, &meta_path, &rt).await?;

        // TODO: persist & load the nodes key

        let key_path = IrohPaths::SecretKey.with_root(&root);
        tokio::fs::create_dir_all(&meta_path).await?;
        let secret_key = load_secret_key(key_path).await?;

        let node = Node::builder(bao_store, doc_store)
            .bind_addr(DEFAULT_BIND_ADDR.into())
            .secret_key(secret_key)
            .derp_mode(iroh::net::derp::DerpMode::Default)
            .peers_data_path(peers_data_path)
            .runtime(&rt)
            .spawn()
            .await?;
        let client = node.client();

        let doc = client.docs.import(ticket).await?;

        let author_path = root.join("author");
        let author = if author_path.exists() {
            let author_raw = tokio::fs::read_to_string(&author_path).await?;
            let author: AuthorId = author_raw.parse()?;
            author
        } else {
            let author = client.authors.create().await?;
            tokio::fs::write(&author_path, author.to_string()).await?;
            author
        };

        Ok(Iroh {
            node,
            client,
            doc,
            author,
        })
    }
}

async fn process_job<C, S>(job: SyndicationJobIroh<C>, iroh: Iroh) -> Result<()>
where
    C: HasMutableSphereContext<S>,
    S: Storage + 'static,
{
    let SyndicationJobIroh { revision, context } = job;
    debug!("Attempting to syndicate version DAG {revision} to iroh");
    let iroh_identity = iroh.node.peer_id();
    let checkpoint_key = format!("syndication/iroh/{iroh_identity}");

    debug!("Iroh node identified as {}", iroh_identity);

    // Take a lock on the `SphereContext` and look up the most recent
    // syndication checkpoint for this iroh node
    let (sphere_revision, syndication_checkpoint, db) = {
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

        (counterpart_revision, syndication_checkpoint, db)
    };

    let timeline = Timeline::new(&db)
        .slice(
            &sphere_revision,
            syndication_checkpoint.last_syndicated_version.as_ref(),
        )
        .to_chronological()
        .await?;

    // For all CIDs since the last historical checkpoint, syndicate a CAR
    // of blocks that are unique to that revision to the backing IPFS
    // implementation
    for root_cid in timeline {
        let block_stream = memo_body_stream(db.clone(), &root_cid, true);
        tokio::pin!(block_stream);
        while let Some(next) = block_stream.next().await {
            let (cid, block) = next?;

            let key = format!("{}/{}", root_cid.to_string(), cid.to_string());
            iroh.doc.set_bytes(iroh.author, key.into(), block).await?;
        }
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
