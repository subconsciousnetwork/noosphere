#![cfg(not(target_arch = "wasm32"))]
#![cfg(test)]
pub mod utils;
use anyhow::Result;
use noosphere_core::{authority::generate_ed25519_key, data::Did, view::SPHERE_LIFETIME};
use noosphere_ns::{
    utils::{generate_capability, generate_fact, wait_for_peers},
    DHTConfig, Multiaddr, NameSystem, NameSystemClient, NsRecord,
};
use noosphere_storage::{derive_cid, MemoryStorage, SphereDb};
use utils::generate_default_listening_address;

use futures::future::try_join_all;
use libipld_cbor::DagCborCodec;
use std::sync::Arc;
use tokio::sync::Mutex;
use ucan::{builder::UcanBuilder, crypto::KeyMaterial, store::UcanJwtStore, time::now, Ucan};
use ucan_key_support::ed25519::Ed25519KeyMaterial;

/// Data related to an owner sphere and a NameSystem running
/// on its behalf in it's corresponding gateway.
struct NSData {
    pub ns: NameSystem,
    pub owner_key: Ed25519KeyMaterial,
    pub owner_id: Did,
    pub sphere_id: Did,
    pub delegation: Ucan,
}

async fn generate_name_system(
    store: &mut SphereDb<MemoryStorage>,
    bootstrap_addresses: &[Multiaddr],
) -> Result<NSData> {
    let owner_key = generate_ed25519_key();
    let owner_id = Did(owner_key.get_did().await?);
    let sphere_key = generate_ed25519_key();
    let sphere_id = Did(sphere_key.get_did().await?);

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
    let _ = store.write_token(&delegation.encode()?).await?;

    let ns_key = generate_ed25519_key();
    let ns = NameSystem::new(&ns_key, store.to_owned(), DHTConfig::default())?;
    ns.listen(generate_default_listening_address()).await?;
    ns.add_peers(bootstrap_addresses.to_vec()).await?;
    ns.bootstrap().await?;
    Ok(NSData {
        ns,
        owner_key,
        owner_id,
        sphere_id,
        delegation,
    })
}

/// Generates a DHT network bootstrap node with `ns_count`
/// NameSystems connected, each with a corresponding owner sphere.
async fn generate_name_systems_network(
    ns_count: usize,
) -> Result<(NameSystem, SphereDb<MemoryStorage>, Vec<NSData>)> {
    let mut store = SphereDb::new(&MemoryStorage::default()).await?;
    let mut name_systems: Vec<NSData> = vec![];

    let bootstrap_node = {
        let key = generate_ed25519_key();
        let node = NameSystem::new(&key, store.clone(), DHTConfig::default())?;
        node.listen(generate_default_listening_address()).await?;
        node
    };
    let address = bootstrap_node.address().await?.unwrap();
    let bootstrap_addresses = vec![address];

    for _ in 0..ns_count {
        let ns_data = generate_name_system(&mut store, &bootstrap_addresses).await?;
        name_systems.push(ns_data);
    }

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

    let [ns_1, ns_2] = [ns_data.remove(0), ns_data.remove(0)];

    // Test propagating records from ns_1 to ns_2
    ns_1.ns
        .put_record(
            UcanBuilder::default()
                .issued_by(&ns_1.owner_key)
                .for_audience(&ns_1.sphere_id)
                .claiming_capability(&generate_capability(&ns_1.sphere_id))
                .with_fact(generate_fact(&sphere_1_cid_1.to_string()))
                .witnessed_by(&ns_1.delegation)
                .with_expiration(*ns_1.delegation.expires_at())
                .build()?
                .sign()
                .await?
                .into(),
        )
        .await?;

    // `None` for a record that cannot be found
    assert!(
        ns_2.ns.get_record(&Did::from("unknown")).await?.is_none(),
        "no record found"
    );

    // Baseline fetching record from the network.
    assert_eq!(
        ns_2.ns
            .get_record(&ns_1.sphere_id)
            .await?
            .expect("to be some")
            .link()
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
                .claiming_capability(&generate_capability(&ns_1.sphere_id))
                .with_fact(generate_fact(&sphere_1_cid_2.to_string()))
                .witnessed_by(&ns_1.delegation)
                .with_expiration(*ns_1.delegation.expires_at())
                .build()?
                .sign()
                .await?
                .into(),
        )
        .await?;

    let temp_identity = Did(generate_ed25519_key().get_did().await?);
    assert!(!ns_2.ns.flush_records_for_identity(&temp_identity).await);
    assert!(ns_2.ns.flush_records_for_identity(&ns_1.sphere_id).await);
    assert_eq!(
        ns_2.ns
            .get_record(&ns_1.sphere_id)
            .await?
            .expect("to be some")
            .link()
            .unwrap(),
        &sphere_1_cid_2,
        "latest record is found from network after flushing record"
    );

    // Store an expired record in ns_1's cache
    ns_1.ns.get_cache().await.insert(
        ns_2.owner_id.clone(),
        UcanBuilder::default()
            .issued_by(&ns_2.owner_key)
            .for_audience(&ns_2.sphere_id)
            .claiming_capability(&generate_capability(&ns_2.sphere_id))
            .with_fact(generate_fact(&sphere_2_cid_1.to_string()))
            .witnessed_by(&ns_2.delegation)
            .with_expiration(now() - 1000) // already expired
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
                .claiming_capability(&generate_capability(&ns_2.sphere_id))
                .with_fact(generate_fact(&sphere_2_cid_2.to_string()))
                .witnessed_by(&ns_2.delegation)
                .with_expiration(*ns_2.delegation.expires_at())
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
            .link()
            .unwrap(),
        &sphere_2_cid_2,
        "non-cached record found for sphere_2"
    );

    Ok(())
}

#[test_log::test(tokio::test)]
async fn test_name_system_validation() -> Result<()> {
    let (_bootstrap_node, _store, mut ns_data) = generate_name_systems_network(1).await?;
    let [ns_1] = [ns_data.remove(0)];

    let sphere_1_cid_1 = derive_cid::<DagCborCodec>(b"00000000");

    assert!(
        ns_1.ns
            .put_record(
                UcanBuilder::default()
                    .issued_by(&ns_1.owner_key)
                    .for_audience(&ns_1.sphere_id)
                    .claiming_capability(&generate_capability(&ns_1.sphere_id))
                    .with_fact(generate_fact(&sphere_1_cid_1.to_string()))
                    .witnessed_by(&ns_1.delegation)
                    .with_expiration(now() - 1000) // already expired
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

#[test_log::test(tokio::test)]
async fn it_is_thread_safe() -> Result<()> {
    let (_bootstrap_node, _store, mut ns_data) = generate_name_systems_network(1).await?;
    let [ns_1] = [ns_data.remove(0)];
    let address = derive_cid::<DagCborCodec>(b"00000000");

    let ucan_record: NsRecord = UcanBuilder::default()
        .issued_by(&ns_1.owner_key)
        .for_audience(&ns_1.sphere_id)
        .claiming_capability(&generate_capability(&ns_1.sphere_id))
        .with_fact(generate_fact(&address.to_string()))
        .witnessed_by(&ns_1.delegation)
        .with_expiration(*ns_1.delegation.expires_at())
        .build()?
        .sign()
        .await?
        .into();

    // Store a dummy record for this name system's own owner sphere
    ns_1.ns
        .get_cache()
        .await
        .insert(ns_1.owner_id.clone(), ucan_record.clone());

    wait_for_peers(&ns_1.ns, 1).await?;
    let network_info = ns_1.ns.network_info().await?;
    assert_eq!(network_info.num_peers, 1, "expected number of peers");

    let arc_ns = Arc::new(Mutex::new(ns_1.ns));
    let mut join_handles = vec![];
    for _ in 0..10 {
        let name_system = arc_ns.clone();
        let identity = ns_1.owner_id.clone();
        let record = ucan_record.clone();
        join_handles.push(tokio::spawn(async move {
            let ns = name_system.lock().await;
            ns.put_record(record).await.unwrap();
            let record = ns.get_record(&identity).await.unwrap();
            let network_info = ns.network_info().await.unwrap();
            (record, network_info)
        }));
    }
    for (record, n_info) in try_join_all(join_handles).await? {
        assert_eq!(record.unwrap().link().unwrap(), &address);
        assert_eq!(n_info, network_info);
    }

    Ok(())
}
