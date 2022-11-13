use std::{collections::BTreeSet, io::Cursor, path::PathBuf, sync::Arc};

use anyhow::{anyhow, Result};
use cid::Cid;
use noosphere_core::{authority::Author, data::Did, view::Sphere};
use noosphere_fs::SphereFs;
use noosphere_storage::{db::SphereDb, interface::Store};
use tokio::sync::Mutex;
use tokio_stream::StreamExt;

use crate::{html::transform::SubtextToHtmlTransformer, write::WriteTarget};

use super::transform::SphereToHtmlTransformer;

static DEFAULT_STYLES: &[u8] = include_bytes!("./static/styles.css");

/// Given a sphere identity, storage and a WriteTarget implementation, produce
/// rendered HTML output up to and including the complete historical revisions
/// of the slug-named content of the sphere.
pub async fn sphere_into_html<S, W>(
    sphere_identity: &Did,
    db: &SphereDb<S>,
    write_target: &W,
) -> Result<()>
where
    S: Store + 'static,
    W: WriteTarget + 'static,
{
    let mut next_sphere_cid = match db.get_version(sphere_identity).await? {
        Some(link) => Some(link),
        _ => {
            return Err(anyhow!(
                "Could not resolve CID for sphere {}",
                sphere_identity
            ))
        }
    };

    let write_target = Arc::new(write_target.clone());
    let mut latest_revision = true;
    let author = Author::anonymous();

    while let Some(sphere_cid) = next_sphere_cid {
        let sphere_index: PathBuf = format!("permalink/{}/index.html", sphere_cid).into();

        // We write the sphere index last, so if we already have it we can
        // assume this revision has been written in the past
        // TODO(#54): Figure out how to enable forced-regeneration of some-or-all
        // of history that has been generated before
        if write_target.exists(&sphere_index).await? {
            break;
        }

        let write_actions = Arc::new(Mutex::new(BTreeSet::<Cid>::new()));
        let sphere = Sphere::at(&sphere_cid, db);
        let links = sphere.try_get_links().await?;
        let mut link_stream = links.stream().await?;

        let mut tasks = Vec::new();

        while let Some(Ok((slug, cid))) = link_stream.next().await {
            let file_name: PathBuf = format!("permalink/{}/index.html", cid).into();

            // Skip this write entirely if the content has been written
            // TODO(#55): This may not hold in a world where there are multiple
            // files written per slug; an example might be a video file that
            // needs to be transformed into an HTML document to present the
            // video, and the video file itself.
            if write_target.exists(&file_name).await? {
                continue;
            }

            tasks.push(W::spawn({
                let slug = slug.clone();
                let cid = *cid;
                let write_actions = write_actions.clone();
                let write_target = write_target.clone();
                let sphere_identity = sphere_identity.clone();
                let db = db.clone();
                let author = author.clone();
                let latest_revision = latest_revision;

                async move {
                    {
                        let mut write_actions = write_actions.lock().await;

                        // Skip this write, to cover the case where we have
                        // multiple slugs referring to the same CID (and the
                        // write is already being handled by another task).
                        if write_actions.contains(&cid) {
                            return Ok(());
                        } else {
                            write_actions.insert(cid);
                        }
                    }

                    let fs = SphereFs::at(&sphere_identity, &sphere_cid, &author, &db).await?;

                    let transformer = SubtextToHtmlTransformer::new(&fs);

                    if let Some(html_stream) = transformer.transform(&slug).await? {
                        write_target
                            .write(&file_name, &mut Box::pin(html_stream))
                            .await?;

                        if latest_revision {
                            write_target
                                .symlink(&format!("permalink/{}", cid).into(), &PathBuf::from(slug))
                                .await?;
                        }
                    }

                    // TODO(#56): Support backlinks somehow; probably as a dynamic
                    // widget at the bottom of the HTML document

                    Ok(())
                }
            }));
        }

        // Let all the content writes happen; bail out if any of them fail
        // TODO(#59): Investigate if we should attempt to recover in any of the
        // cases where writing content may fail
        futures::future::try_join_all(tasks).await?;

        let fs = SphereFs::at(sphere_identity, &sphere_cid, &author, db).await?;
        let sphere_transformer = SphereToHtmlTransformer::new(&fs);

        if let Some(read) = sphere_transformer.transform().await? {
            write_target
                .write(&sphere_index, &mut Box::pin(read))
                .await?;

            if latest_revision {
                write_target
                    .symlink(&sphere_index, &PathBuf::from("index.html"))
                    .await?;
            }
        }

        next_sphere_cid = sphere.try_as_memo().await?.parent;
        latest_revision = false;
    }

    // TODO(#57): Writing these static files should be done concurrently
    write_target
        .write(
            &PathBuf::from("theme/styles.css"),
            &mut Cursor::new(DEFAULT_STYLES),
        )
        .await?;

    // TODO(#58): Introduce some kind of default logo
    // write_target
    //     .write(
    //         &PathBuf::from("theme/logo.svg"),
    //         &mut Cursor::new(LOGO_SVG),
    //     )
    //     .await?;

    Ok(())
}

#[cfg(test)]
pub mod tests {
    use std::path::PathBuf;

    use noosphere_core::{
        authority::{generate_ed25519_key, Author},
        data::{ContentType, Did, Header},
        view::Sphere,
    };
    use noosphere_fs::SphereFs;
    use noosphere_storage::{db::SphereDb, memory::MemoryStorageProvider};
    use ucan::crypto::KeyMaterial;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    use crate::write::MemoryWriteTarget;

    use super::sphere_into_html;

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_writes_a_file_from_the_sphere_to_the_target_as_html() {
        // let mut sphere_store = MemoryStore::default();
        // let mut block_store = MemoryStore::default();

        let storage_provider = MemoryStorageProvider::default();
        let mut db = SphereDb::new(&storage_provider).await.unwrap();

        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await.unwrap();

        let (sphere, proof, _) = Sphere::try_generate(&owner_did, &mut db).await.unwrap();

        let sphere_identity = Did(sphere.try_get_identity().await.unwrap());
        let author = Author {
            key: owner_key,
            authorization: Some(proof),
        };

        db.set_version(&sphere_identity, sphere.cid())
            .await
            .unwrap();

        let mut fs = SphereFs::latest(&sphere_identity, &author, &db)
            .await
            .unwrap();

        let cats_cid = fs
            .write(
                "cats",
                &ContentType::Subtext.to_string(),
                b"# Cats\n\n> It is said that cats are /divine creatures\n\nCats [[are]] great\n\n/animals".as_ref(),
                None
            )
            .await
            .unwrap();

        fs.write(
            "animals",
            &ContentType::Subtext.to_string(),
            b"Animals are multicellular, eukaryotic organisms in the biological kingdom Animalia."
                .as_ref(),
            Some(vec![(Header::Title.to_string(), "Animals".into())]),
        )
        .await
        .unwrap();

        fs.save(None).await.unwrap();

        let write_target = MemoryWriteTarget::default();

        sphere_into_html(&sphere_identity, &db, &write_target)
            .await
            .unwrap();

        let bytes = write_target
            .read(&PathBuf::from(format!("permalink/{}/index.html", cats_cid)))
            .await
            .unwrap();

        let html = std::str::from_utf8(&bytes).unwrap();

        assert_eq!(
            html,
            r#"<!doctype html>
<html>
<head><meta charset="utf-8"><title>Untitled note</title><link rel="stylesheet" media="all" href="/theme/styles.css"></head>
<body>
<article role="main" class="noosphere-content" data-content-type="text/subtext">
<ol class="blocks">
<li class="block"><section class="block-content"><h1 class="block-header"><span class="text">Cats</span></h1></section></li>
<li class="block"><section class="block-content"><p class="block-blank"></p></section></li>
<li class="block"><section class="block-content"><blockquote class="block-quote"><span class="text">It is said that cats are </span>
<a href="/divine" class="slashlink">/divine</a>
<span class="text"> creatures</span></blockquote></section><ul class="block-transcludes"><li class="transclude-item"><a class="transclude-format-text" href="/divine"><span class="link-text">/divine</span></a></li></ul></li>
<li class="block"><section class="block-content"><p class="block-blank"></p></section></li>
<li class="block"><section class="block-content"><p class="block-paragraph"><span class="text">Cats </span>
<a href="/are" class="wikilink">[[are]]</a>
<span class="text"> great</span></p></section></li>
<li class="block"><section class="block-content"><p class="block-blank"></p></section></li>
<li class="block"><ul class="block-transcludes"><li class="transclude-item"><a class="transclude-format-text" href="/animals"><span class="title">Animals</span><span class="excerpt">Animals are multicellular, eukaryotic organisms in the biological kingdom Animalia.</span><span class="link-text">/animals</span></a></li></ul></li>
</ol>
</body>
</html>"#
        );
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_symlinks_a_file_slug_to_the_latest_file_version() {
        let storage_provider = MemoryStorageProvider::default();
        let mut db = SphereDb::new(&storage_provider).await.unwrap();

        let owner_key = generate_ed25519_key();
        let owner_did = owner_key.get_did().await.unwrap();

        let (sphere, proof, _) = Sphere::try_generate(&owner_did, &mut db).await.unwrap();

        let sphere_identity = Did(sphere.try_get_identity().await.unwrap());
        let author = Author {
            key: owner_key,
            authorization: Some(proof),
        };

        db.set_version(&sphere_identity, sphere.cid())
            .await
            .unwrap();

        let mut fs = SphereFs::latest(&sphere_identity, &author, &db)
            .await
            .unwrap();

        let _cats_cid = fs
            .write(
                "cats",
                &ContentType::Subtext.to_string(),
                b"# Cats\n\n> It is said that cats are /divine creatures\n\nCats [[are]] great\n\n/animals".as_ref(),
                None,
            )
            .await
            .unwrap();

        let cats_revised_cid = fs
            .write(
                "cats",
                &ContentType::Subtext.to_string(),
                b"Nevermind, I don't like cats".as_ref(),
                None,
            )
            .await
            .unwrap();

        fs.save(None).await.unwrap();

        let write_target = MemoryWriteTarget::default();

        sphere_into_html(&sphere_identity, &db, &write_target)
            .await
            .unwrap();

        let cats_revised_html = write_target
            .read(&PathBuf::from(format!(
                "permalink/{}/index.html",
                cats_revised_cid
            )))
            .await
            .unwrap();

        let symlink_path = write_target
            .resolve_symlink(&PathBuf::from("cats"))
            .await
            .unwrap();

        let cats_slug_html = write_target
            .read(&symlink_path.join("index.html"))
            .await
            .unwrap();

        assert_eq!(cats_revised_html, cats_slug_html);
    }
}
