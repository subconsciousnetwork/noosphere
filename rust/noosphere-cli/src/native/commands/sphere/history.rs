use anyhow::Result;
use noosphere_core::data::MapOperation;
use noosphere_sphere::HasSphereContext;
use tokio_stream::StreamExt;
use ucan::Ucan;

use crate::workspace::Workspace;

/// Print the available local history of the sphere to stdout
pub async fn history(workspace: &Workspace) -> Result<()> {
    let sphere_context = workspace.sphere_context().await?;
    let sphere = sphere_context.to_sphere().await?;
    let db = sphere.store().clone();

    let history_stream = sphere.into_history_stream(None);

    tokio::pin!(history_stream);

    while let Ok(Some((version, sphere))) = history_stream.try_next().await {
        if let Ok(mutation) = sphere.derive_mutation().await {
            let author = mutation.author();

            info!(
                r#"Version: {version}
Author: {author}"#
            );

            if mutation.is_empty() {
                info!("Changes: None\n");
            } else {
                info!("Changes:\n");
            }

            if sphere.to_memo().await?.parent.is_some() {
                let content_changes = mutation.content().changes();

                if !content_changes.is_empty() {
                    let mut removed = Vec::new();
                    for change in content_changes {
                        match change {
                            MapOperation::Add { key, .. } => info!("{: >12}  /{key}", "Modified"),
                            MapOperation::Remove { key } => removed.push(key),
                        }
                    }

                    info!("")
                }

                let petname_changes = mutation.identities().changes();

                if !petname_changes.is_empty() {
                    let mut any_petnames_added = false;
                    let mut removed_petnames = Vec::new();

                    for change in petname_changes {
                        match change {
                            MapOperation::Add {
                                key,
                                value: identity,
                            } => {
                                any_petnames_added = true;
                                info!("{: >12}  {} ({})", "Followed", key, identity.did)
                            }
                            MapOperation::Remove { key } => removed_petnames.push(key),
                        };
                    }

                    if !removed_petnames.is_empty() && any_petnames_added {
                        info!("");
                    }

                    for name in removed_petnames {
                        info!("{: >12}  {}", "Unfollowed", name);
                    }

                    info!("");
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
            } else {
                info!("End of history");
            }
        } else {
            break;
        }
    }

    Ok(())
}
