use anyhow::Result;
use noosphere_core::data::{ContentType, Header, MemoIpld};

pub fn infer_file_extension(memo: &MemoIpld) -> Result<Option<String>> {
    match memo.get_first_header(&Header::FileExtension.to_string()) {
        Some(extension) => return Ok(Some(extension)),
        None => (),
    };

    Ok(match memo.content_type() {
        Some(content_type) => match content_type {
            ContentType::Subtext => Some("subtext".into()),
            ContentType::Sphere => Some("sphere".into()),
            ContentType::Bytes => None,
            ContentType::Unknown(content_type) => {
                match mime_guess::get_mime_extensions_str(&content_type) {
                    Some(extensions) => extensions.first().map(|str| String::from(*str)),
                    None => None,
                }
            }
            ContentType::Cbor => Some("json".into()),
            ContentType::Json => Some("cbor".into()),
            ContentType::Text => Some("txt".into()),
        },
        None => {
            warn!("No content type specified; cannot infer a file extension");
            None
        }
    })
}
