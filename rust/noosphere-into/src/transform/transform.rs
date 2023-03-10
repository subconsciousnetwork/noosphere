use noosphere_sphere::HasSphereContext;
use noosphere_storage::Storage;
use ucan::crypto::KeyMaterial;

use crate::{Resolver, SphereContentTranscluder, StaticHtmlResolver, Transcluder};

/// A [Transform] represents the combination of a [Resolver] and a
/// [Transcluder]. Together these elements form a transformation over
/// some input Noosphere content.
pub trait Transform: Clone {
    type Resolver: Resolver;
    type Transcluder: Transcluder;

    fn resolver(&self) -> &Self::Resolver;
    fn transcluder(&self) -> &Self::Transcluder;
}

/// A [Transform] that is suitable for converting Noosphere content to
/// HTML for a basic state website generator.
#[derive(Clone)]
pub struct StaticHtmlTransform<R, K, S>
where
    R: HasSphereContext<K, S> + Clone,
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    pub resolver: StaticHtmlResolver,
    pub transcluder: SphereContentTranscluder<R, K, S>,
}

impl<R, K, S> StaticHtmlTransform<R, K, S>
where
    R: HasSphereContext<K, S> + Clone,
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    pub fn new(content: R) -> Self {
        StaticHtmlTransform {
            resolver: StaticHtmlResolver(),
            transcluder: SphereContentTranscluder::new(content),
        }
    }
}

impl<R, K, S> Transform for StaticHtmlTransform<R, K, S>
where
    R: HasSphereContext<K, S> + Clone,
    K: KeyMaterial + Clone + 'static,
    S: Storage + 'static,
{
    type Resolver = StaticHtmlResolver;

    type Transcluder = SphereContentTranscluder<R, K, S>;

    fn resolver(&self) -> &Self::Resolver {
        &self.resolver
    }

    fn transcluder(&self) -> &Self::Transcluder {
        &self.transcluder
    }
}
