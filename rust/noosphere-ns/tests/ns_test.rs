#![cfg(not(target_arch = "wasm32"))]
#![cfg(test)]
pub mod utils;
use anyhow::{anyhow, Result};
use futures::future::try_join_all;
use libipld_cbor::DagCborCodec;
use noosphere_core::{authority::generate_ed25519_key, view::SPHERE_LIFETIME};
use noosphere_ns::{
    dht::{DHTNode, DefaultRecordValidator},
    utils::{generate_capability, generate_fact},
    NameSystem, NameSystemBuilder,
};
use noosphere_storage::{
    db::SphereDb, encoding::derive_cid, memory::MemoryStorageProvider, memory::MemoryStore,
};

use ucan::{builder::UcanBuilder, crypto::KeyMaterial, store::UcanJwtStore, time::now, Ucan};
use ucan_key_support::ed25519::Ed25519KeyMaterial;
use utils::create_bootstrap_nodes;

/// Data related to an owner sphere and a NameSystem running
/// on its behalf in it's corresponding gateway.
struct NSData {
    pub ns: NameSystem<MemoryStore>,
    pub owner_key: Ed25519KeyMaterial,
    pub owner_id: String,
    pub sphere_id: String,
    pub delegation: Ucan,
}

/// Generates a DHT network bootstrap node with `ns_count`
/// NameSystems connected, each with a corresponding owner sphere.
async fn generate_name_systems_network(
    ns_count: usize,
) -> Result<(
    DHTNode<DefaultRecordValidator>,
    SphereDb<MemoryStore>,
    Vec<NSData>,
)> {
    let bootstrap_node = create_bootstrap_nodes(1, DHTNode::<DefaultRecordValidator>::validator())
        .map_err(|e| anyhow!(e.to_string()))?
        .pop()
        .unwrap();
    let bootstrap_addresses = vec![bootstrap_node.p2p_address().unwrap().to_owned()];

    let mut store = SphereDb::new(&MemoryStorageProvider::default()).await?;
    let mut name_systems: Vec<NSData> = vec![];

    for _ in 0..ns_count {
        let owner_key = generate_ed25519_key();
        let owner_id = owner_key.get_did().await?;
        let sphere_key = generate_ed25519_key();
        let sphere_id = sphere_key.get_did().await?;

        // Delegate `sphere_key`'s publishing authority to `owner_key`
        let delegate_capability = generate_capability(&sphere_id);
        let delegation = UcanBuilder::default()
            .issued_by(&sphere_key)
            .for_audience(&owner_id)
            .with_lifetime(SPHERE_LIFETIME)
            .claiming_capability(&delegate_capability)
            .build()?
            .sign()
            .await?;
        let _ = &store.write_token(&delegation.encode()?).await?;

        let ns_key = generate_ed25519_key();
        let ns: NameSystem<MemoryStore> = NameSystemBuilder::default()
            .key_material(&ns_key)
            .store(&store)
            .peer_dialing_interval(1)
            .bootstrap_peers(&bootstrap_addresses)
            .build()?;
        name_systems.push(NSData {
            ns,
            owner_key,
            owner_id,
            sphere_id,
            delegation,
        });
    }

    let futures: Vec<_> = name_systems
        .iter_mut()
        .map(|data| data.ns.connect())
        .collect();
    try_join_all(futures).await?;

    Ok((bootstrap_node, store, name_systems))
}

#[test_log::test(tokio::test)]
async fn test_name_system_peer_propagation() -> Result<()> {
    // Create two NameSystems, where `ns_1` is publishing for `sphere_1`
    // and `ns_2` is publishing for `sphere_2`.
    let (_bootstrap_node, _store, mut ns_data) = generate_name_systems_network(2).await?;

    let sphere_1_cid_1 = derive_cid::<DagCborCodec>(b"00000000");
    let sphere_1_cid_2 = derive_cid::<DagCborCodec>(b"11111111");
    let sphere_2_cid_1 = derive_cid::<DagCborCodec>(b"99999999");
    let sphere_2_cid_2 = derive_cid::<DagCborCodec>(b"88888888");

    let [mut ns_1, mut ns_2] = [ns_data.remove(0), ns_data.remove(0)];

    // Test propagating records from ns_1 to ns_2
    ns_1.ns
        .put_record(
            UcanBuilder::default()
                .issued_by(&ns_1.owner_key)
                .for_audience(&ns_1.sphere_id)
                .with_lifetime(SPHERE_LIFETIME - 1000)
                .claiming_capability(&generate_capability(&ns_1.sphere_id))
                .with_fact(generate_fact(&sphere_1_cid_1.to_string()))
                .witnessed_by(&ns_1.delegation)
                .build()?
                .sign()
                .await?
                .into(),
        )
        .await?;

    // `None` for a record that cannot be found
    assert!(
        ns_2.ns.get_record("unknown").await?.is_none(),
        "no record found"
    );

    // Baseline fetching record from the network.
    assert_eq!(
        ns_2.ns
            .get_record(&ns_1.sphere_id)
            .await?
            .expect("to be some")
            .address()
            .unwrap(),
        &sphere_1_cid_1,
        "first record found"
    );

    // Flush records by identity and fetch latest value from network.
    ns_1.ns
        .put_record(
            UcanBuilder::default()
                .issued_by(&ns_1.owner_key)
                .for_audience(&ns_1.sphere_id)
                .with_lifetime(SPHERE_LIFETIME - 1000)
                .claiming_capability(&generate_capability(&ns_1.sphere_id))
                .with_fact(generate_fact(&sphere_1_cid_2.to_string()))
                .witnessed_by(&ns_1.delegation)
                .build()?
                .sign()
                .await?
                .into(),
        )
        .await?;
    assert!(!ns_2
        .ns
        .flush_records_for_identity(&generate_ed25519_key().get_did().await?));
    assert!(ns_2.ns.flush_records_for_identity(&ns_1.sphere_id));
    assert_eq!(
        ns_2.ns
            .get_record(&ns_1.sphere_id)
            .await?
            .expect("to be some")
            .address()
            .unwrap(),
        &sphere_1_cid_2,
        "latest record is found from network after flushing record"
    );

    // Store an expired record in ns_1's cache
    ns_1.ns.get_cache_mut().insert(
        ns_2.owner_id.clone(),
        UcanBuilder::default()
            .issued_by(&ns_2.owner_key)
            .for_audience(&ns_2.sphere_id)
            .with_expiration(now() - 1000) // already expired
            .claiming_capability(&generate_capability(&ns_2.sphere_id))
            .with_fact(generate_fact(&sphere_2_cid_1.to_string()))
            .witnessed_by(&ns_2.delegation)
            .build()?
            .sign()
            .await?
            .into(),
    );

    // Publish an updated record for sphere_2
    ns_2.ns
        .put_record(
            UcanBuilder::default()
                .issued_by(&ns_2.owner_key)
                .for_audience(&ns_2.sphere_id)
                .with_lifetime(SPHERE_LIFETIME - 1000)
                .claiming_capability(&generate_capability(&ns_2.sphere_id))
                .with_fact(generate_fact(&sphere_2_cid_2.to_string()))
                .witnessed_by(&ns_2.delegation)
                .build()?
                .sign()
                .await?
                .into(),
        )
        .await?;

    // Fetch sphere 2's record, which should check the network
    // rather than using the cached, expired record.
    assert_eq!(
        ns_1.ns
            .get_record(&ns_2.sphere_id)
            .await?
            .expect("to be some")
            .address()
            .unwrap(),
        &sphere_2_cid_2,
        "non-cached record found for sphere_2"
    );

    Ok(())
}

#[test_log::test(tokio::test)]
async fn test_name_system_validation() -> Result<()> {
    let (_bootstrap_node, _store, mut ns_data) = generate_name_systems_network(1).await?;
    let [mut ns_1] = [ns_data.remove(0)];

    let sphere_1_cid_1 = derive_cid::<DagCborCodec>(b"00000000");

    // Test propagating records from ns_1 to ns_2
    assert!(
        ns_1.ns
            .put_record(
                UcanBuilder::default()
                    .issued_by(&ns_1.owner_key)
                    .for_audience(&ns_1.sphere_id)
                    .with_expiration(now() - 1000) // already expired
                    .claiming_capability(&generate_capability(&ns_1.sphere_id))
                    .with_fact(generate_fact(&sphere_1_cid_1.to_string()))
                    .witnessed_by(&ns_1.delegation)
                    .build()?
                    .sign()
                    .await?
                    .into(),
            )
            .await
            .is_err(),
        "invalid (expired) records cannot be propagated"
    );
    Ok(())
}
