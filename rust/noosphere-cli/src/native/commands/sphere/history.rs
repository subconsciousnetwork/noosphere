use anyhow::Result;
use noosphere_core::context::HasSphereContext;
use noosphere_core::data::MapOperation;
use tokio_stream::StreamExt;
use ucan::Ucan;

use crate::workspace::Workspace;

/// Print the available local history of the sphere to stdout
pub async fn history(workspace: &Workspace) -> Result<()> {
    let sphere_context = workspace.sphere_context().await?;
    let sphere = sphere_context.to_sphere().await?;
    let latest_version = sphere.cid().clone();
    let db = sphere.store().clone();

    let history_stream = sphere.into_history_stream(None);

    tokio::pin!(history_stream);

    while let Ok(Some((version, sphere))) = history_stream.try_next().await {
        if let Ok(mutation) = sphere.derive_mutation().await {
            let author = mutation.author();
            let has_parent = sphere.to_memo().await?.parent.is_some();

            let version_string = if latest_version == version {
                format!("{version} (Latest)")
            } else {
                format!("{version}")
            };

            info!(
                r#"Version: {version_string}
Author: {author}"#
            );

            if !has_parent {
                info!("This is the base version of the sphere")
            }

            if mutation.is_empty() {
                info!("No content, address book or authority changes\n");
            } else {
                info!("Changes:\n");
            }

            let content_changes = mutation.content().changes();

            if !content_changes.is_empty() {
                let mut removed = Vec::new();
                let mut any_content_added = false;

                for change in content_changes {
                    match change {
                        MapOperation::Add { key, .. } => {
                            any_content_added = true;
                            info!("{: >12}  /{key}", "Modified")
                        }
                        MapOperation::Remove { key } => removed.push(key),
                    }
                }

                if any_content_added {
                    info!("");
                }

                for slug in &removed {
                    info!("{: >12}  /{slug}", "Removed");
                }

                if !removed.is_empty() {
                    info!("")
                }
            }

            let petname_changes = mutation.identities().changes();

            if !petname_changes.is_empty() {
                let mut any_peers_updated = false;
                let mut removed_petnames = Vec::new();
                let mut followed_petnames = Vec::new();

                for change in petname_changes {
                    match change {
                        MapOperation::Add {
                            key,
                            value: identity,
                        } => {
                            if let Some(link_record) = &identity.link_record {
                                any_peers_updated = true;
                                info!("{: >12}  {} -> {}", "Updated", key, link_record);
                            } else {
                                followed_petnames.push((key, &identity.did));
                            }
                        }
                        MapOperation::Remove { key } => removed_petnames.push(key),
                    };
                }

                if any_peers_updated {
                    info!("");
                }

                for (name, link_record) in &followed_petnames {
                    info!("{: >12}  {} -> {}", "Followed", name, link_record);
                }

                if !followed_petnames.is_empty() {
                    info!("");
                }

                for name in &removed_petnames {
                    info!("{: >12}  {}", "Unfollowed", name);
                }

                if !removed_petnames.is_empty() {
                    info!("");
                }
            }

            let delegation_changes = mutation.delegations().changes();

            if !delegation_changes.is_empty() {
                for change in delegation_changes {
                    if let MapOperation::Add { value, .. } = change {
                        let ucan = value.resolve_ucan(&db).await?;
                        let audience = ucan.audience();

                        info!("{: >12}  {} ({})", "Delegated", value.name, audience);
                    }
                }

                info!("");
            }

            let revocation_changes = mutation.revocations().changes();

            if !revocation_changes.is_empty() {
                for change in revocation_changes {
                    if let MapOperation::Add { key, .. } = change {
                        let jwt = key.load_from(&db).await?;
                        let ucan = Ucan::try_from(jwt.as_str())?;
                        let audience = ucan.audience();

                        info!("{: >12}  {}", "Revoked", audience);
                    }
                }

                info!("");
            }

            if !has_parent {
                info!("Start of history");
            }
        } else {
            break;
        }
    }

    Ok(())
}
