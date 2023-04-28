use std::{convert::Infallible, fmt::Display, str::FromStr};

pub enum Header {
    ContentType,
    Proof,
    Author,
    Title,
    Signature,
    Version,
    FileExtension,
    Unknown(String),
}

impl Display for Header {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let value = match self {
            Header::ContentType => "Content-Type",
            Header::Proof => "Proof",
            Header::Author => "Author",
            Header::Title => "Title",
            Header::Signature => "Signature",
            Header::Version => "Version",
            Header::FileExtension => "File-Extension",
            Header::Unknown(name) => name,
        };

        write!(f, "{value}")
    }
}

impl FromStr for Header {
    type Err = Infallible;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s.to_lowercase().as_str() {
            "content-type" => Header::ContentType,
            "file-extension" => Header::FileExtension,
            "proof" => Header::Proof,
            "author" => Header::Author,
            "title" => Header::Title,
            "signature" => Header::Signature,
            "version" => Header::Version,
            _ => Header::Unknown(s.to_string()),
        })
    }
}
