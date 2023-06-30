use std::{convert::TryFrom, str::FromStr};

use anyhow::{anyhow, Result};
use cid::Cid;
use noosphere_core::{
    authority::{generate_capability, SphereAbility},
    data::{DelegationIpld, Link, RevocationIpld},
    view::{Sphere, SphereMutation},
};
use serde_json::{json, Value};
use ucan::{builder::UcanBuilder, crypto::KeyMaterial, store::UcanJwtStore, Ucan};

use tokio_stream::StreamExt;

use crate::native::workspace::Workspace;

pub async fn auth_add(did: &str, name: Option<String>, workspace: &Workspace) -> Result<Cid> {
    workspace.ensure_sphere_initialized()?;
    let sphere_did = workspace.sphere_identity().await?;
    let mut db = workspace.db().await?;

    let latest_sphere_cid = db.require_version(&sphere_did).await?;
    let sphere = Sphere::at(&latest_sphere_cid.into(), &db);

    let authority = sphere.get_authority().await?;
    let delegations = authority.get_delegations().await?;
    let mut delegations_stream = delegations.stream().await?;

    while let Some((Link { cid, .. }, delegation)) = delegations_stream.try_next().await? {
        let ucan = delegation.resolve_ucan(&db).await?;
        let authorized_did = ucan.audience();

        if authorized_did == did {
            return Err(anyhow!(
                r#"{} is already authorized to access the sphere
Here is the identity of the authorization:

  {}

If you want to change it to something new, revoke the current one first:

  orb auth revoke {}

You will be able to add a new one after the old one is revoked"#,
                did,
                cid,
                delegation.name
            ));
        }
    }

    let name = match name {
        Some(name) => name,
        None => {
            let random_name = witty_phrase_generator::WPGen::new()
                .with_words(3)
                .unwrap_or_else(|| vec!["Unnamed"])
                .into_iter()
                .map(String::from)
                .collect::<Vec<String>>()
                .join("-");
            info!(
                "Note: since no name was specified, the authorization will be saved with the generated name \"{random_name}\""
            );
            random_name
        }
    };

    let my_key = workspace.key().await?;
    let my_did = my_key.get_did().await?;
    let latest_sphere_cid = db.require_version(&sphere_did).await?;
    let authorization = workspace.authorization().await?;
    let authorization_expiry: Option<u64> = {
        let ucan = authorization.as_ucan(&db).await?;
        ucan.expires_at().to_owned()
    };

    let mut builder = UcanBuilder::default()
        .issued_by(&my_key)
        .for_audience(did)
        .claiming_capability(&generate_capability(&sphere_did, SphereAbility::Authorize))
        .with_nonce();

    // TODO(ucan-wg/rs-ucan#114): Clean this up when
    // `UcanBuilder::with_expiration` accepts `Option<u64>`
    if let Some(exp) = authorization_expiry {
        builder = builder.with_expiration(exp);
    }

    // TODO(ucan-wg/rs-ucan#32): Clean this up when we can use a CID as an authorization
    // .witnessed_by(&authorization)
    let mut signable = builder.build()?;
    signable
        .proofs
        .push(Cid::try_from(&authorization)?.to_string());

    let jwt = signable.sign().await?.encode()?;

    let delegation = DelegationIpld::register(&name, &jwt, &db).await?;

    let sphere = Sphere::at(&latest_sphere_cid.into(), &db);

    let mut mutation = SphereMutation::new(&my_did);

    mutation
        .delegations_mut()
        .set(&Link::new(delegation.jwt), &delegation);

    let mut revision = sphere.apply_mutation(&mutation).await?;
    let version_cid = revision.sign(&my_key, Some(&authorization)).await?;

    db.set_version(&sphere_did, &version_cid).await?;

    info!(
        r#"Successfully authorized {did} to access your sphere.

IMPORTANT: You MUST sync to enable your gateway to recognize the authorization:

  orb sync

This is the authorization's identity:

  {}
  
Use this identity when joining the sphere on the other client"#,
        delegation.jwt
    );

    Ok(delegation.jwt)
}

pub async fn auth_list(as_json: bool, workspace: &Workspace) -> Result<()> {
    let sphere_did = workspace.sphere_identity().await?;
    let db = workspace.db().await?;

    let latest_sphere_cid = db
        .get_version(&sphere_did)
        .await?
        .ok_or_else(|| anyhow!("Sphere version pointer is missing or corrupted"))?;

    let sphere = Sphere::at(&latest_sphere_cid.into(), &db);

    let authorization = sphere.get_authority().await?;

    let allowed_ucans = authorization.get_delegations().await?;

    let mut authorizations: Vec<(String, String, Cid)> = Vec::new();
    let mut delegation_stream = allowed_ucans.stream().await?;
    let mut max_name_length: usize = 7;

    while let Some(Ok((_, delegation))) = delegation_stream.next().await {
        let jwt = db.require_token(&delegation.jwt).await?;
        let ucan = Ucan::from_str(&jwt)?;
        let name = delegation.name.clone();

        max_name_length = max_name_length.max(name.len());
        authorizations.push((
            delegation.name.clone(),
            ucan.audience().into(),
            delegation.jwt,
        ));
    }

    if as_json {
        let authorizations: Vec<Value> = authorizations
            .into_iter()
            .map(|(name, did, cid)| {
                json!({
                    "name": name,
                    "did": did,
                    "cid": cid.to_string()
                })
            })
            .collect();
        info!("{}", serde_json::to_string_pretty(&json!(authorizations))?);
    } else {
        info!("{:1$}  AUTHORIZED KEY", "NAME", max_name_length);
        for (name, did, _) in authorizations {
            info!("{name:max_name_length$}  {did}");
        }
    }

    Ok(())
}

pub async fn auth_revoke(name: &str, workspace: &Workspace) -> Result<()> {
    workspace.ensure_sphere_initialized()?;
    let sphere_did = workspace.sphere_identity().await?;
    let mut db = workspace.db().await?;

    let latest_sphere_cid = db
        .get_version(&sphere_did)
        .await?
        .ok_or_else(|| anyhow!("Sphere version pointer is missing or corrupted"))?;

    let my_key = workspace.key().await?;
    let my_did = my_key.get_did().await?;

    let sphere = Sphere::at(&latest_sphere_cid.into(), &db);

    let authority = sphere.get_authority().await?;

    let delegations = authority.get_delegations().await?;

    let mut delegation_stream = delegations.stream().await?;

    while let Some(Ok((Link { cid, .. }, delegation))) = delegation_stream.next().await {
        if delegation.name == name {
            let revocation = RevocationIpld::revoke(cid, &my_key).await?;

            let mut mutation = SphereMutation::new(&my_did);

            let key = Link::new(*cid);

            mutation.delegations_mut().remove(&key);
            mutation.revocations_mut().set(&key, &revocation);

            let mut revision = sphere.apply_mutation(&mutation).await?;
            let ucan = workspace.authorization().await?;

            let sphere_cid = revision.sign(&my_key, Some(&ucan)).await?;

            db.set_version(&sphere_did, &sphere_cid).await?;

            info!("The authorization named {name:?} has been revoked");

            return Ok(());
        }
    }

    Err(anyhow!("There is no authorization named {:?}", name))
}
