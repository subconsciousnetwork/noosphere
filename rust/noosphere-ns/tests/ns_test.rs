#![cfg(not(target_arch = "wasm32"))]
#![cfg(test)]
pub mod utils;
use anyhow::{anyhow, Result};
use cid::{
    multihash::{Code, MultihashDigest},
    Cid,
};
use noosphere_core::authority::generate_ed25519_key;
use noosphere_ns::{NameSystem, NameSystemBuilder};

use utils::{create_bootstrap_nodes, wait_ms};

fn new_cid(bytes: &[u8]) -> Cid {
    const RAW: u64 = 0x55;
    Cid::new_v1(RAW, Code::Sha2_256.digest(bytes))
}

#[test_log::test(tokio::test)]
async fn test_name_system() -> Result<()> {
    let bootstrap_node = create_bootstrap_nodes(1)
        .map_err(|e| anyhow!(e.to_string()))?
        .pop()
        .unwrap();
    let bootstrap_addresses = vec![bootstrap_node.p2p_address().unwrap().to_owned()];

    let sphere_1_cid_1 = new_cid(b"00000000");
    let sphere_1_cid_2 = new_cid(b"11111111");
    let sphere_1_cid_3 = new_cid(b"22222222");
    let sphere_1_id = String::from("did:sphere_1");
    let sphere_2_cid_1 = new_cid(b"99999999");
    let sphere_2_cid_2 = new_cid(b"88888888");
    let sphere_2_id = String::from("did:sphere_2");

    let ns_1_key = generate_ed25519_key();
    let ns_2_key = generate_ed25519_key();

    let mut ns_1: NameSystem = NameSystemBuilder::default()
        .key_material(&ns_1_key)
        .listening_port(30000)
        .ttl(3600)
        .peer_dialing_interval(1)
        .bootstrap_peers(&bootstrap_addresses)
        .build()?;

    let mut ns_2: NameSystem = NameSystemBuilder::default()
        .key_material(&ns_2_key)
        .listening_port(30001)
        .ttl(1)
        .peer_dialing_interval(1)
        .bootstrap_peers(&bootstrap_addresses)
        .build()?;

    ns_1.connect().await?;
    ns_2.connect().await?;

    // Test propagating records from ns_1 to ns_2
    ns_1.set_record(&sphere_1_id, &sphere_1_cid_1).await?;

    // `None` for a record that cannot be found
    let cid = ns_2.get_record(&String::from("unknown")).await;
    assert_eq!(cid, None, "no record found");

    // Baseline fetching record from the network.
    let cid = ns_2.get_record(&sphere_1_id).await.expect("to be some");
    assert_eq!(cid, &sphere_1_cid_1, "first record found");

    // Use cache if record is not expired
    ns_1.set_record(&sphere_1_id, &sphere_1_cid_2).await?;
    let cid = ns_2.get_record(&sphere_1_id).await.expect("to be some");
    assert_eq!(cid, &sphere_1_cid_1, "record found in cache");

    // Flush records and fetch latest value from network.
    ns_2.flush_records();
    let cid = ns_2.get_record(&sphere_1_id).await.expect("to be some");
    assert_eq!(
        cid, &sphere_1_cid_2,
        "latest record is found from network after flushing all records"
    );

    // Flush records by identity and fetch latest value from network.
    ns_1.set_record(&sphere_1_id, &sphere_1_cid_3).await?;
    assert!(!ns_2.flush_records_for_identity(&String::from("invalid did")));
    assert!(ns_2.flush_records_for_identity(&sphere_1_id));
    let cid = ns_2.get_record(&sphere_1_id).await.expect("to be some");
    assert_eq!(
        cid, &sphere_1_cid_3,
        "latest record is found from network after flushing record"
    );

    // Now testing propagating records from ns_2 to ns_1,
    // with a much shorter TTL
    ns_2.set_record(&sphere_2_id, &sphere_2_cid_1).await?;

    // Baseline fetching record from the network.
    let cid = ns_1.get_record(&sphere_2_id).await.expect("to be some");
    assert_eq!(cid, &sphere_2_cid_1, "first record found");

    // Fetch record from network if local record is expired.
    ns_2.set_record(&sphere_2_id, &sphere_2_cid_2).await?;
    // Wait to ensure record is expired ;_;
    wait_ms(1000).await;
    let cid = ns_1.get_record(&sphere_2_id).await.expect("to be some");
    assert_eq!(
        cid, &sphere_2_cid_2,
        "record found from network from local expired record"
    );

    Ok(())
}
