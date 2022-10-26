use horrorshow::html;
use noosphere_core::data::{ContentType, Header, MemoIpld};

/// Generate an HTML "envelope" for content described by the given memo.
/// Currently, only Subtext and Sphere content types are explicitly supported.
/// This envelope is made up of two parts because the content inside will be
/// streamed for most content types.
pub fn html_document_envelope(memo: MemoIpld) -> (String, String) {
    let content_type = memo.content_type();

    let title = if let Some(title) = memo.get_first_header(&Header::Title.to_string()) {
        title
    } else {
        match content_type {
            Some(ContentType::Subtext) => "Untitled note",
            Some(ContentType::Sphere) => "My sphere",
            _ => "Untitled",
        }
        .into()
    };

    let head_section = html! {
        head {
            meta(charset="utf-8");

            title: &title;

            link(rel="stylesheet", media="all", href="/theme/styles.css");
        }
    }
    .to_string();

    let content_type_attribute_value = if let Some(content_type) = &content_type {
        content_type.to_string()
    } else {
        "unknown".into()
    };

    let (body_open, body_close) = match content_type {
        Some(ContentType::Subtext) => (r#"<ol class="blocks">"#, "</ol>"),
        Some(ContentType::Sphere) => (r#"<ul class="sphere-transcludes">"#, "</ul>"),
        _ => ("", ""),
    };

    (
        format!(
            r#"<!doctype html>
<html>
{}
<body>
<article role="main" class="noosphere-content" data-content-type="{}">
{}
"#,
            head_section, content_type_attribute_value, body_open
        ),
        format!(
            r#"{}
</body>
</html>"#,
            body_close
        ),
    )
}
