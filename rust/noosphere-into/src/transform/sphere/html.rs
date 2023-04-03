use crate::{html_document_envelope, subtext_to_html_fragment_stream, Transform, TransformStream};
use async_stream::stream;
use futures::Stream;
use noosphere_core::data::{ContentType, Header};
use noosphere_core::view::Sphere;
use noosphere_sphere::{HasSphereContext, SphereFile};
use noosphere_storage::{BlockStore, Storage};
use ucan::crypto::KeyMaterial;

/// Given a [Transform] and a [Sphere], produce a stream that yields the file
/// content as an HTML document
pub fn sphere_to_html_document_stream<C, K, S, T>(
    context: C,
    transform: T,
) -> impl Stream<Item = String>
where
    C: HasSphereContext<K, S>,
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
    T: Transform,
{
    stream! {
        let sphere = match context.to_sphere().await {
            Ok(sphere) => sphere,
            Err(error) => {
                error!("Could not get sphere: {:?}", error);
                return ();
            }
        };

        let sphere_identity = match sphere.get_identity().await {
            Ok(did) => did,
            Err(error) => {
                error!("Could not get sphere identity: {:?}", error);
                return ();
            }
        };

        let mut memo = match sphere.to_memo().await {
            Ok(memo) => memo,
            Err(error) => {
                error!("Could not get sphere memo: {:?}", error);
                return ();
            }
        };

        let (html_prefix, html_suffix) = html_document_envelope(&memo);

        memo.replace_first_header(&Header::ContentType.to_string(), &ContentType::Subtext.to_string());

        let sphere_file = SphereFile {
            sphere_identity,
            sphere_version: sphere.cid().clone(),
            memo_version: sphere.cid().clone(),
            memo,
            contents: TransformStream(sphere_to_subtext_stream(sphere)).into_reader(),
        };

        let fragment_stream = subtext_to_html_fragment_stream(sphere_file, transform);

        yield html_prefix;

        for await fragment_part in fragment_stream {
            yield fragment_part;
        }

        yield html_suffix;
    }
}

pub fn sphere_to_subtext_stream<S>(sphere: Sphere<S>) -> impl Stream<Item = String>
where
    S: BlockStore + 'static,
{
    stream! {
        let links = match sphere.get_content().await {
            Ok(links) => links,
            Err(error) => {
                warn!("Could not resolve links for sphere: {}", error);
                return;
            }
        };

        let link_stream = match links.into_stream().await {
            Ok(stream) => stream,
            Err(error) => {
                warn!("Could not stream links for sphere: {}", error);
                return;
            }
        };

        for await link in link_stream {
            match link {
                Ok((slug, _)) => yield format!("/{slug}\n"),
                Err(error) => {
                    warn!("Failed to stream sphere link: {}", error);
                    continue;
                }
            }
        }
    }
}
