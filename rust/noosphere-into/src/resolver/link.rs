use std::fmt::Display;

use subtext::Slashlink;

#[cfg(doc)]
use crate::Resolver;
#[cfg(doc)]
use crate::Transcluder;

/// This enum represents the resolved value that may be returned by a [Resolver]
/// and is provided to a [Transcluder]
pub enum ResolvedLink {
    Hyperlink { href: String },
    Slashlink { link: Slashlink, href: String },
}

impl Display for ResolvedLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let href = match self {
            ResolvedLink::Hyperlink { href } => href,
            ResolvedLink::Slashlink { href, .. } => href,
        };
        write!(f, "{}", href)
    }
}
