use std::{collections::BTreeSet, io::Cursor, path::PathBuf, sync::Arc};

use anyhow::{anyhow, Result};
use cid::Cid;
use libipld_cbor::DagCborCodec;
use noosphere_sphere::{HasSphereContext, SphereContentRead, SphereCursor};
use noosphere_storage::{block_serialize, Storage};
use tokio::sync::Mutex;
use tokio_stream::StreamExt;
use ucan::crypto::KeyMaterial;

use crate::{
    file_to_html_stream, sphere_to_html_document_stream, HtmlOutput, StaticHtmlTransform,
    TransformStream, WriteTarget,
};

static DEFAULT_STYLES: &[u8] = include_bytes!("./static/styles.css");

/// Given a sphere [Did], [SphereDb] and a [WriteTarget], produce rendered HTML
/// output up to and including the complete historical revisions of the
/// slug-named content of the sphere.
pub async fn sphere_into_html<C, K, S, W>(sphere_context: C, write_target: &W) -> Result<()>
where
    C: HasSphereContext<K, S> + 'static,
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
    W: WriteTarget + 'static,
{
    let mut next_sphere_cid = Some(sphere_context.version().await?);

    let mut cursor = SphereCursor::latest(sphere_context);

    let write_target = Arc::new(write_target.clone());
    let mut latest_revision = true;

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
        let sphere = cursor.to_sphere().await?;
        let links = sphere.get_links().await?;
        let mut link_stream = links.stream().await?;

        let mut tasks = Vec::new();

        while let Some(Ok((slug, memo))) = link_stream.next().await {
            let (cid, _) = block_serialize::<DagCborCodec, _>(memo)?;
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
                let write_actions = write_actions.clone();
                let write_target = write_target.clone();
                let latest_revision = latest_revision;
                let cursor = cursor.clone();

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

                    let sphere_file = cursor
                        .read(&slug)
                        .await?
                        .ok_or_else(|| anyhow!("No file found for {}", slug))?;

                    let transform = StaticHtmlTransform::new(cursor.clone());
                    let reader = TransformStream(file_to_html_stream(
                        sphere_file,
                        HtmlOutput::Document,
                        transform,
                    ))
                    .into_reader();

                    write_target.write(&file_name, reader).await?;

                    if latest_revision {
                        write_target
                            .symlink(&format!("permalink/{}", cid).into(), &PathBuf::from(slug))
                            .await?;
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

        // let cursor = SphereCursor::at(sphere_context, sphere_cid);

        let transform = StaticHtmlTransform::new(cursor.clone());
        let reader = TransformStream(sphere_to_html_document_stream(cursor.clone(), transform))
            .into_reader();

        write_target.write(&sphere_index, reader).await?;

        if latest_revision {
            write_target
                .symlink(&sphere_index, &PathBuf::from("index.html"))
                .await?;
        }

        next_sphere_cid = cursor.rewind().await?;
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

    use noosphere_core::data::{ContentType, Header};

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    use crate::write::MemoryWriteTarget;
    use noosphere_sphere::{
        helpers::{simulated_sphere_context, SimulationAccess},
        HasMutableSphereContext, SphereContentWrite, SphereCursor,
    };

    use super::sphere_into_html;

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_writes_a_file_from_the_sphere_to_the_target_as_html() {
        let context = simulated_sphere_context(SimulationAccess::ReadWrite)
            .await
            .unwrap();
        let mut cursor = SphereCursor::latest(context);

        let cats_cid = cursor
            .write(
                "cats",
                &ContentType::Subtext.to_string(),
                b"# Cats\n\n> It is said that cats are /divine creatures\n\nCats [[are]] great\n\n/animals".as_ref(),
                None
            )
            .await
            .unwrap();

        cursor.write(
            "animals",
            &ContentType::Subtext.to_string(),
            b"Animals are multicellular, eukaryotic organisms in the biological kingdom Animalia."
                .as_ref(),
            Some(vec![(Header::Title.to_string(), "Animals".into())]),
        )
        .await
        .unwrap();

        cursor.save(None).await.unwrap();

        let write_target = MemoryWriteTarget::default();

        sphere_into_html(cursor, &write_target).await.unwrap();

        let bytes = write_target
            .read(&PathBuf::from(format!("permalink/{}/index.html", cats_cid)))
            .await
            .unwrap();

        let html = std::str::from_utf8(&bytes).unwrap();

        println!();
        println!("{}", html);
        println!();

        let expected = r#"<!doctype html>
<html>
<head><meta charset="utf-8"><title>Untitled note</title><link rel="stylesheet" media="all" href="/theme/styles.css"></head>
<body>
<article role="main" class="noosphere-content" data-content-type="text/subtext">
<ol class="blocks">
<article class="subtext"><section class="block"><section class="block-content"><h1 class="block-header"><span class="text">Cats</span></h1></section></section>

<section class="block"><section class="block-content"><blockquote class="block-quote"><span class="text">It is said that cats are </span>
<a href="/divine" class="slashlink">/divine</a>
<span class="text"> creatures</span></blockquote></section><section class="block-transcludes"><aside class="transclude"><a class="transclude-format-text" href="/divine"><span class="link-text">/divine</span></a></aside></section></section>

<section class="block"><section class="block-content"><p class="block-paragraph"><span class="text">Cats </span>
<a href="/are" class="wikilink"><span class="wikilink-open-bracket">[[</span><span class="wikilink-text">are</span><span class="wikilink-close-bracket">]]</span></a>
<span class="text"> great</span></p></section></section>

<section class="block"><section class="block-transcludes"><aside class="transclude"><a class="transclude-format-text" href="/animals"><span class="title">Animals</span><span class="excerpt">Animals are multicellular, eukaryotic organisms in the biological kingdom Animalia.</span><span class="link-text">/animals</span></a></aside></section></section>
</article></ol>
</body>
</html>"#;

        assert_eq!(html, expected);
    }

    #[cfg_attr(target_arch = "wasm32", wasm_bindgen_test)]
    #[cfg_attr(not(target_arch = "wasm32"), tokio::test)]
    async fn it_symlinks_a_file_slug_to_the_latest_file_version() {
        let context = simulated_sphere_context(SimulationAccess::ReadWrite)
            .await
            .unwrap();
        let mut cursor = SphereCursor::latest(context);

        let _cats_cid = cursor
            .write(
                "cats",
                &ContentType::Subtext.to_string(),
                b"# Cats\n\n> It is said that cats are /divine creatures\n\nCats [[are]] great\n\n/animals".as_ref(),
                None,
            )
            .await
            .unwrap();

        let cats_revised_cid = cursor
            .write(
                "cats",
                &ContentType::Subtext.to_string(),
                b"Nevermind, I don't like cats".as_ref(),
                None,
            )
            .await
            .unwrap();

        cursor.save(None).await.unwrap();

        let write_target = MemoryWriteTarget::default();

        sphere_into_html(cursor, &write_target).await.unwrap();

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
