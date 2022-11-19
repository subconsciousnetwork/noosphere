use std::sync::Arc;

use anyhow::Result;
use noosphere::sphere::SphereContext;
use noosphere_core::{
    data::{AddressIpld, Did, Jwt},
    view::SphereMutation,
};
use noosphere_ns::{DHTKeyMaterial, Multiaddr, NSRecord, NameSystem, NameSystemBuilder};
use noosphere_storage::Storage;
use tokio::{
    sync::{
        mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender},
        Mutex,
    },
    task::JoinHandle,
};

pub enum NSJob {
    PutRecord { publish_token: NSRecord },

    GetRecord { sphere_id: Did, pet_name: String },
}

pub fn start_ns_service<K, S>(
    gateway_context: Arc<Mutex<SphereContext<K, S>>>,
    bootstrap_peers: &[Multiaddr],
    ns_port: Option<u16>,
) -> (UnboundedSender<NSJob>, JoinHandle<Result<()>>)
where
    K: DHTKeyMaterial + 'static,
    S: Storage + 'static,
{
    let (tx, rx) = unbounded_channel();

    (
        tx,
        tokio::task::spawn(ns_worker(
            gateway_context,
            bootstrap_peers.to_owned(),
            ns_port,
            rx,
        )),
    )
}

async fn ns_worker<K, S>(
    gateway_context: Arc<Mutex<SphereContext<K, S>>>,
    bootstrap_peers: Vec<Multiaddr>,
    ns_port: Option<u16>,
    mut receiver: UnboundedReceiver<NSJob>,
) -> Result<()>
where
    K: DHTKeyMaterial + 'static,
    S: Storage + 'static,
{
    let name_system: NameSystem = {
        let ctx = gateway_context.lock().await;
        let db = ctx.db();
        let gateway_key = &ctx.author().key;

        let mut builder = NameSystemBuilder::default()
            .key_material(gateway_key)
            .store(db)
            .bootstrap_peers(&bootstrap_peers);
        
        if let Some(port) = ns_port {
            builder = builder.listening_port(port);
        }

        let ns = builder.build().await?;
        ns.bootstrap().await?;
        ns
    };

    while let Some(job) = receiver.recv().await {
        match job {
            NSJob::PutRecord { publish_token } => {
                if let Err(err) = name_system.put_record(publish_token.clone()).await {
                    warn!(
                        "Error publishing a name system record with {:#?}: {:#?}.",
                        publish_token, err
                    );
                }
            }
            NSJob::GetRecord {
                pet_name,
                sphere_id,
            } => match name_system.get_record(&sphere_id).await {
                Ok(Some(record)) => {
                    let ctx = gateway_context.lock().await;
                    if let Err(e) = store_ns_record(&ctx, &pet_name, record).await {
                        error!("{:#?}", e);
                    }
                }
                Err(err) => {
                    warn!(
                        "Error fetching a name system record for {:#?}: {:#?}.",
                        sphere_id, err
                    );
                }
                Ok(None) => {
                    warn!("Name system cannot resolve revision for {:#?}.", sphere_id);
                }
            },
        }
    }

    Ok(())
}

async fn store_ns_record<K, S>(
    gateway_context: &SphereContext<K, S>,
    pet_name: &String,
    record: NSRecord,
) -> Result<()>
where
    K: DHTKeyMaterial + 'static,
    S: Storage + 'static,
{
    let sphere = gateway_context.sphere().await?;
    let jwt_payload = Some(Jwt(record.try_to_string()?));

    let address = AddressIpld {
        identity: Did(record.identity().to_owned()),
        last_known_record: jwt_payload,
    };

    let mut mutation = SphereMutation::new(&gateway_context.identity());
    mutation.names_mut().set(&pet_name, &address);
    sphere.try_apply_mutation(&mutation).await?;

    Ok(())
}
