use noosphere_fs::SphereFs;
use noosphere_storage::Storage;
use ucan::crypto::KeyMaterial;

use crate::{Resolver, SphereFsTranscluder, StaticHtmlResolver, Transcluder};

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
pub struct StaticHtmlTransform<S, K>
where
    S: Storage,
    K: KeyMaterial + Clone + 'static,
{
    pub resolver: StaticHtmlResolver,
    pub transcluder: SphereFsTranscluder<S, K>,
}

impl<S, K> StaticHtmlTransform<S, K>
where
    S: Storage,
    K: KeyMaterial + Clone + 'static,
{
    pub fn new(fs: SphereFs<S, K>) -> Self {
        StaticHtmlTransform {
            resolver: StaticHtmlResolver(),
            transcluder: SphereFsTranscluder::new(fs),
        }
    }
}

impl<S, K> Transform for StaticHtmlTransform<S, K>
where
    S: Storage,
    K: KeyMaterial + Clone + 'static,
{
    type Resolver = StaticHtmlResolver;

    type Transcluder = SphereFsTranscluder<S, K>;

    fn resolver(&self) -> &Self::Resolver {
        &self.resolver
    }

    fn transcluder(&self) -> &Self::Transcluder {
        &self.transcluder
    }
}
