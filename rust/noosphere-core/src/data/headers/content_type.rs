use std::{convert::Infallible, fmt::Display, str::FromStr};

#[derive(Ord, PartialOrd, Eq, PartialEq, Clone, Debug)]
pub enum ContentType {
    Text,
    Subtext,
    Sphere,
    Bytes,
    Cbor,
    Json,
    Unknown(String),
}

impl Display for ContentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            ContentType::Text => "text/plain",
            ContentType::Subtext => "text/subtext",
            ContentType::Sphere => "noo/sphere",
            ContentType::Bytes => "raw/bytes",
            ContentType::Cbor => "application/cbor",
            ContentType::Json => "application/json",
            ContentType::Unknown(header) => header.as_str(),
        };

        write!(f, "{value}")
    }
}

impl FromStr for ContentType {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "text/plain" => ContentType::Text,
            "text/subtext" => ContentType::Subtext,
            "noo/sphere" => ContentType::Sphere,
            "raw/bytes" => ContentType::Bytes,
            "application/json" => ContentType::Json,
            "application/cbor" => ContentType::Cbor,
            _ => ContentType::Unknown(String::from(s)),
        })
    }
}
