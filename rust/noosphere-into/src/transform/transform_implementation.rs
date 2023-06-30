use noosphere_sphere::HasSphereContext;
use noosphere_storage::Storage;

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
pub struct StaticHtmlTransform<R, S>
where
    R: HasSphereContext<S> + Clone,
    S: Storage + 'static,
{
    pub resolver: StaticHtmlResolver,
    pub transcluder: SphereContentTranscluder<R, S>,
}

impl<R, S> StaticHtmlTransform<R, S>
where
    R: HasSphereContext<S> + Clone,
    S: Storage + 'static,
{
    pub fn new(content: R) -> Self {
        StaticHtmlTransform {
            resolver: StaticHtmlResolver(),
            transcluder: SphereContentTranscluder::new(content),
        }
    }
}

impl<R, S> Transform for StaticHtmlTransform<R, S>
where
    R: HasSphereContext<S> + Clone,
    S: Storage + 'static,
{
    type Resolver = StaticHtmlResolver;

    type Transcluder = SphereContentTranscluder<R, S>;

    fn resolver(&self) -> &Self::Resolver {
        &self.resolver
    }

    fn transcluder(&self) -> &Self::Transcluder {
        &self.transcluder
    }
}
