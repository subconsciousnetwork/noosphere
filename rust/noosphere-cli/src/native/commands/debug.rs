use std::collections::{BTreeSet, VecDeque};

use crate::workspace::Workspace;
use anyhow::Result;
use cid::Cid;
use noosphere_core::{context::HasSphereContext, view::Sphere};
use noosphere_ipfs::debug::debug_block as debug_block_impl;
use noosphere_storage::{BlockStore, SphereDb};

pub async fn debug_dag(workspace: &Workspace) -> Result<()> {
    workspace.ensure_sphere_initialized()?;

    let sphere_context = workspace.sphere_context().await?;

    let sphere: Sphere<_> = sphere_context.to_sphere().await?;

    let version = sphere.cid();
    let memo = sphere.to_memo().await?;

    info!("Version: {}", version);
    info!("Memo: {:#?}", memo);

    // let version = sphere_context.sphere_context().await?.version().await?;

    Ok(())
}

pub async fn debug_block(cid: &Cid, workspace: &Workspace) -> Result<()> {
    workspace.ensure_sphere_initialized()?;

    let sphere_context = workspace.sphere_context().await?;

    let db: SphereDb<_> = sphere_context.sphere_context().await?.db().clone();
    let block: Vec<u8> = db.require_block(cid).await?;

    let mut out = String::new();

    debug_block_impl(cid, &block, &mut out)?;

    info!("{out}");

    Ok(())
}

pub async fn debug_references(cid: &Cid, workspace: &Workspace) -> Result<()> {
    workspace.ensure_sphere_initialized()?;

    let sphere_context = workspace.sphere_context().await?;
    let root = sphere_context.version().await?;
    let db: SphereDb<_> = sphere_context.sphere_context().await?.db().clone();

    let mut queue = VecDeque::new();
    queue.push_front(Cid::from(root));
    let mut references = BTreeSet::new();

    while let Some(referencer) = queue.pop_back() {
        if let Some(links) = db.get_block_links(&referencer).await? {
            for link in links {
                if &link == cid {
                    references.insert(referencer);
                    continue;
                } else {
                    queue.push_front(link);
                }
            }
        }
    }

    for link in references {
        info!("{link}");
    }

    Ok(())
}
