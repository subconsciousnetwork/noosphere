use std::{convert::Infallible, fmt::Display, ops::Deref, str::FromStr};

pub enum Header {
    ContentType,
    Proof,
    Author,
    Title,
    Signature,
    Version,
    FileExtension,
    LamportOrder,
    Unknown(String),
}

impl Display for Header {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", Deref::deref(self))
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
            "lamport-order" => Header::LamportOrder,
            _ => Header::Unknown(s.to_string()),
        })
    }
}

impl Deref for Header {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        match self {
            Header::ContentType => "Content-Type",
            Header::Proof => "Proof",
            Header::Author => "Author",
            Header::Title => "Title",
            Header::Signature => "Signature",
            Header::Version => "Version",
            Header::FileExtension => "File-Extension",
            Header::LamportOrder => "Lamport-Order",
            Header::Unknown(name) => name.as_str(),
        }
    }
}
