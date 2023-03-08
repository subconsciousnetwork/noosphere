use anyhow::Result;
use libp2p::Multiaddr;
use noosphere_core::{authority::generate_ed25519_key, data::Did, view::SPHERE_LIFETIME};
use noosphere_storage::{MemoryStorage, SphereDb};
use ucan::{builder::UcanBuilder, crypto::KeyMaterial, store::UcanJwtStore, Ucan};
use ucan_key_support::ed25519::Ed25519KeyMaterial;

use crate::{
    dht::RecordValidator, utils::generate_capability, DhtConfig, NameSystem, NameSystemClient,
};
/// Data related to an owner sphere and a NameSystem running
/// on its behalf in it's corresponding gateway.
pub struct NsData {
    pub ns: NameSystem,
    pub owner_key: Ed25519KeyMaterial,
    pub owner_id: Did,
    pub sphere_id: Did,
    pub delegation: Ucan,
}

pub async fn generate_name_system<V: RecordValidator + 'static>(
    store: &mut SphereDb<MemoryStorage>,
    bootstrap_addresses: &[Multiaddr],
    validator: V,
) -> Result<NsData> {
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
    let ns = NameSystem::new(&ns_key, DhtConfig::default(), Some(validator))?;
    ns.listen(generate_default_listening_address()).await?;
    ns.add_peers(bootstrap_addresses.to_vec()).await?;
    ns.bootstrap().await?;
    Ok(NsData {
        ns,
        owner_key,
        owner_id,
        sphere_id,
        delegation,
    })
}

/// Generates a DHT network bootstrap node with `ns_count`
/// NameSystems connected, each with a corresponding owner sphere.
pub async fn generate_name_systems_network<V: RecordValidator + Clone + 'static>(
    ns_count: usize,
    mut store: SphereDb<MemoryStorage>,
    validator: V,
) -> Result<(NameSystem, SphereDb<MemoryStorage>, Vec<NsData>)> {
    let mut name_systems: Vec<NsData> = vec![];

    let bootstrap_node = {
        let key = generate_ed25519_key();
        let node = NameSystem::new(&key, DhtConfig::default(), Some(validator.clone()))?;
        node.listen(generate_default_listening_address()).await?;
        node
    };
    let address = bootstrap_node.address().await?.unwrap();
    let bootstrap_addresses = vec![address];

    for _ in 0..ns_count {
        let ns_data =
            generate_name_system(&mut store, &bootstrap_addresses, validator.clone()).await?;
        name_systems.push(ns_data);
    }

    Ok((bootstrap_node, store, name_systems))
}

pub fn generate_default_listening_address() -> Multiaddr {
    "/ip4/127.0.0.1/tcp/0".parse().expect("parseable")
}
