use std::{convert::TryFrom, str::FromStr};

use anyhow::{anyhow, Result};
use cid::Cid;
use noosphere_core::{
    authority::{SphereAction, SphereReference},
    data::{CidKey, DelegationIpld, RevocationIpld},
    view::{Sphere, SphereMutation},
};
use serde_json::{json, Value};
use ucan::{
    builder::UcanBuilder,
    capability::{Capability, Resource, With},
    crypto::KeyMaterial,
    store::UcanJwtStore,
    Ucan,
};

use tokio_stream::StreamExt;

use crate::native::workspace::Workspace;

pub async fn auth_add(did: &str, name: Option<String>, workspace: &Workspace) -> Result<Cid> {
    let sphere_did = workspace.sphere_identity().await?;
    let mut db = workspace.db().await?;

    let latest_sphere_cid = db.require_version(&sphere_did).await?;
    let sphere = Sphere::at(&latest_sphere_cid, &db);

    let authority = sphere.get_authority().await?;
    let allowed_ucans = authority.try_get_allowed_ucans().await?;
    let mut allowed_stream = allowed_ucans.stream().await?;

    while let Some((CidKey(cid), delegation)) = allowed_stream.try_next().await? {
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
            println!(
                "Note: since no name was specified, the authorization will be saved with the generated name \"{}\"",
                random_name
            );
            random_name
        }
    };

    let my_key = workspace.key().await?;
    let my_did = my_key.get_did().await?;
    let latest_sphere_cid = db.require_version(&sphere_did).await?;
    let authorization = workspace.authorization().await?;
    let authorization_expiry: u64 = {
        let ucan = authorization.resolve_ucan(&db).await?;
        *ucan.expires_at()
    };

    let mut signable = UcanBuilder::default()
        .issued_by(&my_key)
        .for_audience(did)
        .claiming_capability(&Capability {
            with: With::Resource {
                kind: Resource::Scoped(SphereReference {
                    did: sphere_did.to_string(),
                }),
            },
            can: SphereAction::Authorize,
        })
        .with_expiration(authorization_expiry)
        .with_nonce()
        // TODO(ucan-wg/rs-ucan#32): Clean this up when we can use a CID as an authorization
        // .witnessed_by(&authorization)
        .build()?;

    signable
        .proofs
        .push(Cid::try_from(&authorization)?.to_string());

    let jwt = signable.sign().await?.encode()?;

    let delegation = DelegationIpld::try_register(&name, &jwt, &mut db).await?;

    let sphere = Sphere::at(&latest_sphere_cid, &db);

    let mut mutation = SphereMutation::new(&my_did);

    mutation
        .allowed_ucans_mut()
        .set(&CidKey(delegation.jwt), &delegation);

    let mut revision = sphere.apply_mutation(&mutation).await?;
    let version_cid = revision.try_sign(&my_key, Some(&authorization)).await?;

    db.set_version(&sphere_did, &version_cid).await?;

    println!(
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

    let sphere = Sphere::at(&latest_sphere_cid, &db);

    let authorization = sphere.get_authority().await?;

    let allowed_ucans = authorization.try_get_allowed_ucans().await?;

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
        println!("{}", serde_json::to_string_pretty(&json!(authorizations))?);
    } else {
        println!("{:1$}  AUTHORIZED KEY", "NAME", max_name_length);
        for (name, did, _) in authorizations {
            println!("{:1$}  {did}", name, max_name_length);
        }
    }

    Ok(())
}

pub async fn auth_revoke(name: &str, workspace: &Workspace) -> Result<()> {
    let sphere_did = workspace.sphere_identity().await?;
    let mut db = workspace.db().await?;

    let latest_sphere_cid = db
        .get_version(&sphere_did)
        .await?
        .ok_or_else(|| anyhow!("Sphere version pointer is missing or corrupted"))?;

    let my_key = workspace.key().await?;
    let my_did = my_key.get_did().await?;

    let sphere = Sphere::at(&latest_sphere_cid, &db);

    let authorization = sphere.get_authority().await?;

    let allowed_ucans = authorization.try_get_allowed_ucans().await?;

    let mut delegation_stream = allowed_ucans.stream().await?;

    while let Some(Ok((CidKey(cid), delegation))) = delegation_stream.next().await {
        if delegation.name == name {
            let revocation = RevocationIpld::try_revoke(cid, &my_key).await?;

            let mut mutation = SphereMutation::new(&my_did);

            let key = CidKey(*cid);

            mutation.allowed_ucans_mut().remove(&key);
            mutation.revoked_ucans_mut().set(&key, &revocation);

            let mut revision = sphere.apply_mutation(&mutation).await?;
            let ucan = workspace.authorization().await?;

            let sphere_cid = revision.try_sign(&my_key, Some(&ucan)).await?;

            db.set_version(&sphere_did, &sphere_cid).await?;

            println!("The authorization named {:?} has been revoked", name);

            return Ok(());
        }
    }

    Err(anyhow!("There is no authorization named {:?}", name))
}
