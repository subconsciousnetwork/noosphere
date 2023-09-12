use std::{collections::BTreeMap, convert::TryFrom};

use anyhow::{anyhow, Result};
use cid::Cid;
use noosphere_core::context::{
    HasSphereContext, SphereAuthorityRead, SphereAuthorityWrite, SphereWalker,
};
use noosphere_core::data::{Did, Jwt, Link};
use serde_json::{json, Value};
use ucan::{store::UcanJwtStore, Ucan};

use tokio_stream::StreamExt;

use crate::native::{commands::sphere::save, workspace::Workspace};

/// Add an authorization for another device key (given by its [Did]) to the sphere
pub async fn auth_add(did: &Did, name: Option<String>, workspace: &Workspace) -> Result<Cid> {
    workspace.ensure_sphere_initialized()?;

    let mut sphere_context = workspace.sphere_context().await?;

    let current_authorization = sphere_context.get_authorization(did).await?;

    if let Some(authorization) = current_authorization {
        let cid = Cid::try_from(authorization)?;

        return Err(anyhow!(
            r#"{} is already authorized to access the sphere
Here is the identity of the authorization:

  {}

If you want to change it to something new, revoke the current one first:

  orb auth revoke {}

You will be able to add a new one after the old one is revoked"#,
            did,
            cid,
            cid
        ));
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

    let new_authorization = sphere_context.authorize(&name, did).await?;
    let new_authorization_cid = Cid::try_from(new_authorization)?;

    save(None, workspace).await?;

    info!(
        r#"Successfully authorized {did} to access your sphere.

IMPORTANT: You MUST sync to enable your gateway to recognize the authorization:

  orb sync

This is the authorization's identity:

  {new_authorization_cid}
  
Use this identity when joining the sphere on the other client"#
    );

    Ok(new_authorization_cid)
}

fn draw_branch(
    mut items: Vec<Link<Jwt>>,
    mut indentation: Vec<bool>,
    hierarchy: &BTreeMap<Link<Jwt>, Vec<Link<Jwt>>>,
    meta: &BTreeMap<Link<Jwt>, (String, Did, Jwt)>,
) {
    let prefix = indentation
        .iter()
        .enumerate()
        .map(|(index, is_last)| match is_last {
            false if index == 0 => "│",
            false => "   │ ",
            true if index == 0 => " ",
            true => "    ",
        })
        .collect::<String>();

    while let Some(link) = items.pop() {
        let is_last = items.is_empty();

        if let Some((name, id, _token)) = meta.get(&link) {
            let (branch_char, trunk_char) = match is_last {
                true => ('└', ' '),
                false => ('├', '│'),
            };

            info!("{prefix}{}── {}", branch_char, name);
            info!("{prefix}{}   {}", trunk_char, id);

            if let Some(children) = hierarchy.get(&link) {
                info!("{prefix}{}   │", trunk_char);
                indentation.push(is_last);
                draw_branch(children.clone(), indentation.clone(), hierarchy, meta);
            } else {
                info!("{prefix}{}", trunk_char);
            }
        }
    }
}

/// List all authorizations in the sphere, optionally as a tree hierarchy
/// representing the chain of authority, and optionally in JSON format
pub async fn auth_list(tree: bool, as_json: bool, workspace: &Workspace) -> Result<()> {
    let sphere_context = workspace.sphere_context().await?;
    let sphere_identity = sphere_context.identity().await?;
    let db = sphere_context.lock().await.db().clone();

    let walker = SphereWalker::from(&sphere_context);

    let authorization_stream = walker.authorization_stream();

    tokio::pin!(authorization_stream);

    let mut authorization_meta: BTreeMap<Link<Jwt>, (String, Did, Jwt)> = BTreeMap::default();
    let mut max_name_length: usize = 7;

    while let Some((name, identity, link)) = authorization_stream.try_next().await? {
        let jwt = Jwt(db.require_token(&link).await?);
        max_name_length = max_name_length.max(name.len());
        authorization_meta.insert(link.clone(), (name, identity, jwt));
    }

    if tree {
        let mut authorization_roots = Vec::<Link<Jwt>>::new();
        let mut authorization_hierarchy: BTreeMap<Link<Jwt>, Vec<Link<Jwt>>> = BTreeMap::default();

        for (link, (_name, _identity, jwt)) in authorization_meta.iter() {
            let ucan = Ucan::try_from(jwt.as_str())?;

            // TODO(#552): Maybe only consider Noosphere-related proofs here
            // TODO(#553): Maybe filter on proofs that specifically refer to the current sphere
            let proofs = ucan
                .proofs()
                .clone()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|cid| Cid::try_from(cid.as_str()).ok().map(Link::from))
                .collect::<Vec<Link<Jwt>>>();

            if *ucan.issuer() == sphere_identity {
                // TODO(#554): Such an authorization ought not have any topical proofs,
                // but perhaps we should verify that
                authorization_roots.push(link.clone())
            } else {
                for proof in proofs {
                    let items = match authorization_hierarchy.get_mut(&proof) {
                        Some(items) => items,
                        None => {
                            authorization_hierarchy.insert(proof.clone(), Vec::new());
                            authorization_hierarchy.get_mut(&proof).ok_or_else(|| {
                                anyhow!(
                                    "Could not access list of child authorizations for {}",
                                    &proof
                                )
                            })?
                        }
                    };
                    items.push(link.clone());
                }
            }
        }

        if as_json {
            let mut hierarchy = BTreeMap::new();
            for (proof, children) in authorization_hierarchy {
                hierarchy.insert(
                    proof.to_string(),
                    children
                        .iter()
                        .map(|child| child.to_string())
                        .collect::<Vec<String>>(),
                );
            }
            let mut meta = BTreeMap::new();
            for (link, (name, id, token)) in authorization_meta {
                meta.insert(
                    link.to_string(),
                    json!({
                        "name": name,
                        "id": id,
                        "token": token
                    }),
                );
            }

            let roots = authorization_roots
                .into_iter()
                .map(|root| root.to_string())
                .collect::<Vec<String>>();

            info!(
                "{}",
                serde_json::to_string_pretty(&json!({
                    "sphere": sphere_identity,
                    "roots": roots,
                    "hierarchy": hierarchy,
                    "meta": meta
                }))?
            );
        } else {
            info!(" ⌀ Sphere");
            info!(" │ {sphere_identity}");
            info!(" │");

            while let Some(root) = authorization_roots.pop() {
                let items = vec![root];
                let indentation = vec![authorization_roots.is_empty()];

                draw_branch(
                    items,
                    indentation,
                    &authorization_hierarchy,
                    &authorization_meta,
                );
            }
        }
    } else if as_json {
        let authorizations: Vec<Value> = authorization_meta
            .into_iter()
            .map(|(link, (name, did, token))| {
                json!({
                    "name": name,
                    "id": did,
                    "link": link.to_string(),
                    "token": token
                })
            })
            .collect();
        info!("{}", serde_json::to_string_pretty(&json!(authorizations))?);
    } else {
        info!("{:1$}  AUTHORIZED KEY", "NAME", max_name_length);
        for (_, (name, did, _)) in authorization_meta {
            info!("{name:max_name_length$}  {did}");
        }
    }

    Ok(())
}

/// Revoke an authorization for another device key by its nickname
pub async fn auth_revoke(name: &str, workspace: &Workspace) -> Result<()> {
    workspace.ensure_sphere_initialized()?;

    let mut sphere_context = workspace.sphere_context().await?;
    let authorizations = sphere_context.get_authorizations_by_name(name).await?;

    if authorizations.is_empty() {
        return Err(anyhow!("There is no authorization named {:?}", name));
    }

    for authorization in authorizations.iter() {
        sphere_context.revoke_authorization(authorization).await?;
    }

    save(None, workspace).await?;

    Ok(())
}
