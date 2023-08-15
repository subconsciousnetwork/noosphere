use std::{convert::Infallible, fmt::Display, ops::Deref, str::FromStr};

#[derive(Ord, PartialOrd, Eq, PartialEq, Clone, Debug)]
pub enum ContentType {
    Text,
    Subtext,
    Markdown,
    Sphere,
    Bytes,
    Cbor,
    Json,
    Unknown(String),
}

impl Display for ContentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", Deref::deref(self))
    }
}

impl FromStr for ContentType {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "text/plain" => ContentType::Text,
            // See: https://github.com/subconsciousnetwork/subtext/issues/54
            "text/vnd.subtext" => ContentType::Subtext,
            "text/subtext" => ContentType::Subtext,
            "text/x.subtext" => ContentType::Subtext,
            "text/x-subtext" => ContentType::Subtext,
            "text/markdown" => ContentType::Markdown,
            "noo/sphere" => ContentType::Sphere,
            "application/vnd.noosphere.sphere" => ContentType::Sphere,
            "raw/bytes" => ContentType::Bytes,
            "application/json" => ContentType::Json,
            "application/cbor" => ContentType::Cbor,
            _ => ContentType::Unknown(String::from(s)),
        })
    }
}

impl Deref for ContentType {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        match self {
            ContentType::Text => "text/plain",
            ContentType::Markdown => "text/markdown",
            ContentType::Subtext => "text/vnd.subtext",
            ContentType::Sphere => "application/vnd.noosphere.sphere",
            ContentType::Bytes => "raw/bytes",
            ContentType::Cbor => "application/cbor",
            ContentType::Json => "application/json",
            ContentType::Unknown(content_type) => content_type.as_str(),
        }
    }
}
