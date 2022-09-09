use std::{collections::BTreeSet, path::PathBuf, sync::Arc};

use anyhow::{anyhow, Result};
use cid::Cid;
use noosphere::{data::ReferenceIpld, view::Sphere};
use noosphere_fs::SphereFs;
use noosphere_storage::interface::{KeyValueStore, Store};
use tokio::sync::Mutex;
use tokio_stream::StreamExt;

use crate::write::WriteTarget;

use super::SubtextToHtmlTransform;

static DEFAULT_STYLES: &str = include_str!("./static/styles.css");

pub async fn sphere_into_html<S, W>(
    sphere_identity: &str,
    sphere_store: &S,
    block_store: &S,
    write_target: &W,
) -> Result<()>
where
    S: Store + 'static,
    W: WriteTarget + 'static,
{
    let mut next_sphere_cid = match sphere_store.get(sphere_identity).await? {
        Some(ReferenceIpld { link }) => Some(link),
        _ => {
            return Err(anyhow!(
                "Could not resolve CID for sphere {}",
                sphere_identity
            ))
        }
    };

    let write_target = Arc::new(write_target.clone());

    while let Some(sphere_cid) = next_sphere_cid {
        let sphere_index: PathBuf = format!("{}.html", sphere_cid.to_string()).into();

        if write_target.exists(&sphere_index).await? {
            break;
        }

        let write_actions = Arc::new(Mutex::new(BTreeSet::<Cid>::new()));
        let sphere = Sphere::at(&sphere_cid, block_store);
        let links = sphere.try_get_links().await?;
        let mut link_stream = links.stream().await?;

        let mut tasks = Vec::new();

        while let Some(Ok((key, value))) = link_stream.next().await {
            let file_name: PathBuf = format!("{}.html", value.to_string()).into();

            if write_target.exists(&file_name).await? {
                continue;
            }

            tasks.push(W::spawn({
                let key = key.clone();
                let value = value.clone();
                let write_actions = write_actions.clone();
                let write_target = write_target.clone();
                let sphere_identity = sphere_identity.to_string();
                let block_store = block_store.clone();
                let sphere_store = sphere_store.clone();

                async move {
                    {
                        let mut write_actions = write_actions.lock().await;

                        if write_actions.contains(&value) {
                            return Ok(());
                        } else {
                            write_actions.insert(value.clone());
                        }
                    }

                    let fs =
                        SphereFs::at(&sphere_identity, &sphere_cid, &block_store, &sphere_store)?
                            .ok_or_else(|| {
                            anyhow!(
                                "Unable to find revision for sphere {}: {}",
                                sphere_identity,
                                value
                            )
                        })?;

                    let mut sphere_file = fs.read(&key).await?.ok_or_else(|| {
                        anyhow!(
                            "File contents for slug {} ({}) - occurring in sphere {} ({}) - are missing",
                            key,
                            value,
                            sphere_identity,
                            sphere_cid
                        )
                    })?;

                    // write_target.write(&file_name, &mut Box::pin(
                    //     SubtextToHtmlTransform::new(&block_store).transform(&mut sphere_file.contents).await)
                    // ).await?;

                    Ok(())
                }
            }));
        }

        futures::future::try_join_all(tasks).await?;

        next_sphere_cid = sphere.try_as_memo().await?.parent
    }

    Ok(())
}

#[cfg(test)]
pub mod tests {
    use std::path::PathBuf;

    use noosphere::{
        authority::generate_ed25519_key,
        data::{ContentType, ReferenceIpld},
        view::Sphere,
    };
    use noosphere_fs::SphereFs;
    use noosphere_storage::{interface::KeyValueStore, memory::MemoryStore};
    use ucan::crypto::KeyMaterial;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    use crate::write::MemoryWriteTarget;

    use super::sphere_into_html;

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_writes_a_file_from_the_sphere_to_the_target_as_html() {
        let mut sphere_store = MemoryStore::default();
        let mut block_store = MemoryStore::default();

        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await.unwrap();

        let (sphere, proof, _) = Sphere::try_generate(&owner_did, &mut block_store)
            .await
            .unwrap();

        let sphere_identity = sphere.try_get_identity().await.unwrap();

        sphere_store
            .set(
                &sphere_identity,
                &ReferenceIpld {
                    link: sphere.cid().clone(),
                },
            )
            .await
            .unwrap();

        let mut fs = SphereFs::latest(&sphere_identity, &block_store, &sphere_store)
            .await
            .unwrap();

        let cats_cid = fs
            .write(
                "cats",
                &ContentType::Subtext.to_string(),
                b"Cats are great".as_ref(),
                &owner_key,
                Some(&proof),
                None,
            )
            .await
            .unwrap();

        let write_target = MemoryWriteTarget::default();

        sphere_into_html(&sphere_identity, &sphere_store, &block_store, &write_target)
            .await
            .unwrap();

        let bytes = write_target
            .read(&PathBuf::from(format!("{}.html", cats_cid.to_string())))
            .await
            .unwrap();

        assert_eq!(b"<!doctype html><html><head><title>Hello, World</title></head><body>Paragraph</body></html>", bytes.as_slice());
    }
}
