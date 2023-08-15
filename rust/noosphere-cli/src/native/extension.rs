//! Helpers for dealing with translation between slugs and files

use std::str::FromStr;

use anyhow::Result;
use noosphere_core::data::{ContentType, Header, MemoIpld};

/// Given a [MemoIpld], attempt to infer a file extension. A 'File-Extension' header
/// will be used (if present); otherwise, the extension will be inferred from
/// the 'Content-Type' header (again, if present).
pub fn infer_file_extension(memo: &MemoIpld) -> Result<Option<String>> {
    if let Some(extension) = memo.get_first_header(&Header::FileExtension) {
        return Ok(Some(extension));
    }

    Ok(match memo.content_type() {
        Some(content_type) => match content_type {
            ContentType::Subtext => Some("subtext".into()),
            ContentType::Markdown => Some("md".into()),
            ContentType::Sphere => Some("sphere".into()),
            ContentType::Bytes => None,
            ContentType::Unknown(content_type) => {
                match mime_guess::get_mime_extensions_str(&content_type) {
                    Some(extensions) => extensions.first().map(|str| String::from(*str)),
                    None => None,
                }
            }
            ContentType::Cbor => Some("cbor".into()),
            ContentType::Json => Some("json".into()),
            ContentType::Text => Some("txt".into()),
        },
        None => {
            warn!("No content type specified; cannot infer a file extension");
            None
        }
    })
}

/// Given a file extension, infer its mime
pub fn infer_content_type(extension: &str) -> Result<ContentType> {
    // TODO(#558): User-specified/customized extension->mime mapping
    Ok(match extension {
        "subtext" => ContentType::Subtext,
        "sphere" => ContentType::Sphere,
        "cbor" => ContentType::Cbor,
        _ => ContentType::from_str(
            mime_guess::from_ext(extension)
                .first_raw()
                .unwrap_or("raw/bytes"),
        )?,
    })
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use cid::Cid;
    use noosphere_core::data::{ContentType, Header, MemoIpld};

    use crate::extension::infer_content_type;

    use super::infer_file_extension;

    #[cfg(target_arch = "wasm32")]
    use wasm_bindgen_test::wasm_bindgen_test;

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_browser);

    fn scaffold_memo(content_type: &str, extension: Option<&str>) -> MemoIpld {
        let mut memo = MemoIpld {
            parent: None,
            headers: vec![(Header::ContentType.to_string(), content_type.into())],
            body: Cid::default(),
        };

        if let Some(extension) = extension {
            memo.headers
                .push((Header::FileExtension.to_string(), extension.into()));
        }

        memo
    }

    #[test]
    pub fn it_converts_between_common_extensions_and_content_types() -> Result<()> {
        let test_cases = [
            (ContentType::Text, "txt"),
            (ContentType::Cbor, "cbor"),
            (ContentType::Json, "json"),
            (ContentType::Subtext, "subtext"),
            (ContentType::Markdown, "md"),
            (ContentType::Sphere, "sphere"),
        ];

        for (content_type, extension) in test_cases {
            let memo = scaffold_memo(&content_type, None);
            assert_eq!(infer_file_extension(&memo)?, Some(extension.into()));
            assert_eq!(infer_content_type(extension)?, content_type);
        }

        Ok(())
    }
}
