#![cfg(not(target_arch = "wasm32"))]
#![cfg(test)]

use anyhow::Result;
use cid::Cid;
use noosphere_core::{
    authority::{generate_capability, generate_ed25519_key, SphereAbility},
    data::{Did, LINK_RECORD_FACT_NAME},
    tracing::initialize_tracing,
    view::SPHERE_LIFETIME,
};
use noosphere_ns::{helpers::NameSystemNetwork, DhtClient};
use noosphere_storage::{derive_cid, MemoryStorage, SphereDb};

use libipld_cbor::DagCborCodec;
use ucan::{builder::UcanBuilder, crypto::KeyMaterial, store::UcanJwtStore, time::now, Ucan};
use ucan_key_support::ed25519::Ed25519KeyMaterial;

/// Data related to an owner and managed sphere identities
/// and the publishing tokens it can issue.
struct PseudoSphere {
    pub owner_key: Ed25519KeyMaterial,
    #[allow(unused)]
    pub owner_id: Did,
    pub sphere_id: Did,
    pub delegation: Ucan,
}

impl PseudoSphere {
    pub async fn new() -> Result<Self> {
        let owner_key = generate_ed25519_key();
        let owner_id = Did(owner_key.get_did().await?);
        let sphere_key = generate_ed25519_key();
        let sphere_id = Did(sphere_key.get_did().await?);

        // Delegate `sphere_key`'s publishing authority to `owner_key`
        let delegate_capability = generate_capability(&sphere_id, SphereAbility::Publish);
        let delegation = UcanBuilder::default()
            .issued_by(&sphere_key)
            .for_audience(&owner_id)
            .with_lifetime(SPHERE_LIFETIME)
            .claiming_capability(&delegate_capability)
            .build()?
            .sign()
            .await?;

        Ok(PseudoSphere {
            owner_key,
            owner_id,
            sphere_id,
            delegation,
        })
    }

    pub fn generate_record(&self, cid: Cid) -> UcanBuilder<Ed25519KeyMaterial> {
        UcanBuilder::default()
            .issued_by(&self.owner_key)
            .for_audience(&self.sphere_id)
            .claiming_capability(&generate_capability(
                &self.sphere_id,
                SphereAbility::Publish,
            ))
            .with_fact(LINK_RECORD_FACT_NAME, cid.to_string())
            .witnessed_by(&self.delegation, None)
    }

    pub async fn write_proofs_to_store<S: UcanJwtStore>(&self, store: &mut S) -> Result<()> {
        store.write_token(&self.delegation.encode()?).await?;
        Ok(())
    }
}

#[tokio::test]
async fn test_name_system_peer_propagation() -> Result<()> {
    initialize_tracing(None);
    // Create two NameSystems, where `ns_1` is publishing for `sphere_1`
    // and `ns_2` is publishing for `sphere_2`.
    let mut db = SphereDb::new(MemoryStorage::default()).await?;
    let network = NameSystemNetwork::generate(3, Some(db.clone())).await?;
    let sphere_1_cid_1 = derive_cid::<DagCborCodec>(b"00000000");
    let sphere_1_cid_2 = derive_cid::<DagCborCodec>(b"11111111");
    let sphere_2_cid_1 = derive_cid::<DagCborCodec>(b"99999999");

    let ns_1 = network.get(1).unwrap();
    let ns_2 = network.get(2).unwrap();
    let sphere_1 = PseudoSphere::new().await?;
    let sphere_2 = PseudoSphere::new().await?;
    sphere_1.write_proofs_to_store(&mut db).await?;
    sphere_2.write_proofs_to_store(&mut db).await?;

    // Test propagating records from ns_1 to ns_2
    ns_1.put_record(
        sphere_1
            .generate_record(sphere_1_cid_1)
            .with_expiration(sphere_1.delegation.expires_at().unwrap())
            .build()?
            .sign()
            .await?
            .try_into()?,
        1,
    )
    .await?;

    // `None` for a record that cannot be found
    assert!(
        ns_2.get_record(&Did::from("unknown")).await?.is_none(),
        "no record found"
    );

    // Baseline fetching record from the network.
    assert_eq!(
        ns_2.get_record(&sphere_1.sphere_id)
            .await?
            .expect("to be some")
            .get_link()
            .unwrap(),
        sphere_1_cid_1.into(),
        "first record found"
    );

    // Flush records by identity and fetch latest value from network.
    ns_1.put_record(
        sphere_1
            .generate_record(sphere_1_cid_2)
            .with_expiration(sphere_1.delegation.expires_at().unwrap())
            .build()?
            .sign()
            .await?
            .try_into()?,
        1,
    )
    .await?;

    assert_eq!(
        ns_2.get_record(&sphere_1.sphere_id)
            .await?
            .expect("to be some")
            .get_link()
            .unwrap(),
        sphere_1_cid_2.into(),
        "latest record is found from network"
    );

    // Publish an updated record for sphere_2
    ns_2.put_record(
        sphere_2
            .generate_record(sphere_2_cid_1)
            .with_expiration(sphere_2.delegation.expires_at().unwrap())
            .build()?
            .sign()
            .await?
            .try_into()?,
        1,
    )
    .await?;

    // Fetch sphere 2's record, which should check the network
    // rather than using the cached, expired record.
    assert_eq!(
        ns_1.get_record(&sphere_2.sphere_id)
            .await?
            .expect("to be some")
            .get_link()
            .unwrap(),
        sphere_2_cid_1.into(),
        "non-cached record found for sphere_2"
    );

    Ok(())
}

#[tokio::test]
async fn test_name_system_validation() -> Result<()> {
    initialize_tracing(None);
    let mut db = SphereDb::new(MemoryStorage::default()).await?;
    let network = NameSystemNetwork::generate(2, Some(db.clone())).await?;

    let ns_1 = network.get(1).unwrap();
    let sphere_1 = PseudoSphere::new().await?;
    sphere_1.write_proofs_to_store(&mut db).await?;

    let sphere_1_cid_1 = derive_cid::<DagCborCodec>(b"00000000");

    assert!(
        ns_1.put_record(
            sphere_1
                .generate_record(sphere_1_cid_1)
                .with_expiration(now() - 1000) // already expired
                .build()?
                .sign()
                .await?
                .try_into()?,
            1
        )
        .await
        .is_ok(),
        "expired records can be propagated"
    );
    Ok(())
}
