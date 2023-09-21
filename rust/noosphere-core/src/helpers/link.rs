use anyhow::Result;
use noosphere_storage::{MemoryStorage, SphereDb};
use ucan::{builder::UcanBuilder, crypto::KeyMaterial, store::UcanJwtStore};

use crate::{
    authority::{generate_capability, generate_ed25519_key, SphereAbility},
    data::{Did, Link, LinkRecord, LINK_RECORD_FACT_NAME},
    view::Sphere,
};

/// Make a valid link record that represents a sphere "in the distance." The
/// link record and its proof are both put into the provided [UcanJwtStore]
pub async fn make_valid_link_record<S>(store: &mut S) -> Result<(Did, LinkRecord, Link<LinkRecord>)>
where
    S: UcanJwtStore,
{
    let owner_key = generate_ed25519_key();
    let owner_did = owner_key.get_did().await?;
    let mut db = SphereDb::new(MemoryStorage::default()).await?;

    let (sphere, proof, _) = Sphere::generate(&owner_did, &mut db).await?;
    let ucan_proof = proof.as_ucan(&db).await?;

    let sphere_identity = sphere.get_identity().await?;

    let link_record = LinkRecord::from(
        UcanBuilder::default()
            .issued_by(&owner_key)
            .for_audience(&sphere_identity)
            .witnessed_by(&ucan_proof, None)
            .claiming_capability(&generate_capability(
                &sphere_identity,
                SphereAbility::Publish,
            ))
            .with_lifetime(120)
            .with_fact(LINK_RECORD_FACT_NAME, sphere.cid().to_string())
            .build()?
            .sign()
            .await?,
    );

    store.write_token(&ucan_proof.encode()?).await?;
    let link = Link::from(store.write_token(&link_record.encode()?).await?);

    Ok((sphere_identity, link_record, link))
}
