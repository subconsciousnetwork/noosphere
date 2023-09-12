use std::{convert::Infallible, fmt::Display, ops::Deref, str::FromStr};

/// Well-known headers in the Noosphere
pub enum Header {
    /// Content-type, for mimes
    ContentType,
    /// A proof, typically a UCAN JWT
    Proof,
    /// The author's DID
    Author,
    /// A title for the associated content body
    Title,
    /// A signature by the author's key
    Signature,
    /// The Noosphere protocol version
    Version,
    /// A file extension to use when rendering the content to
    /// the file system
    FileExtension,
    /// The logical order relative to any ancestors
    LamportOrder,
    /// All others
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
