use super::fixtures::{EmailSemantics, Identities};
use crate::{
    builder::UcanBuilder, capability::CapabilitySemantics,
    key_material::ed25519::Ed25519KeyMaterial,
};
use anyhow::Result;
use serde::{de::DeserializeOwned, Serialize};
use serde_ipld_dagcbor::{from_slice, to_vec};

pub fn dag_cbor_roundtrip<T>(data: &T) -> Result<T>
where
    T: Serialize + DeserializeOwned,
{
    Ok(from_slice(&to_vec(data)?)?)
}

pub async fn scaffold_ucan_builder(
    identities: &Identities,
) -> Result<UcanBuilder<Ed25519KeyMaterial>> {
    let email_semantics = EmailSemantics {};
    let send_email_as_bob = email_semantics
        .parse("mailto:bob@email.com".into(), "email/send".into(), None)
        .unwrap();
    let send_email_as_alice = email_semantics
        .parse("mailto:alice@email.com".into(), "email/send".into(), None)
        .unwrap();

    let leaf_ucan_alice = UcanBuilder::default()
        .issued_by(&identities.alice_key)
        .for_audience(identities.mallory_did.as_str())
        .with_expiration(1664232146010)
        .claiming_capability(&send_email_as_alice)
        .build()
        .unwrap()
        .sign()
        .await
        .unwrap();

    let leaf_ucan_bob = UcanBuilder::default()
        .issued_by(&identities.bob_key)
        .for_audience(identities.mallory_did.as_str())
        .with_expiration(1664232146010)
        .claiming_capability(&send_email_as_bob)
        .build()
        .unwrap()
        .sign()
        .await
        .unwrap();

    let builder = UcanBuilder::default()
        .issued_by(&identities.mallory_key)
        .for_audience(identities.alice_did.as_str())
        .with_expiration(1664232146010)
        .witnessed_by(&leaf_ucan_alice, None)
        .witnessed_by(&leaf_ucan_bob, None)
        .claiming_capability(&send_email_as_alice)
        .claiming_capability(&send_email_as_bob);

    Ok(builder)
}
