use anyhow::Result;
use horrorshow::html;

pub fn document_prefix() -> String {
    format!(
        r#"<!doctype html>
<html>
{}
<body>"#,
        document_head()
    )
}

pub fn document_head() -> String {
    format!(
        "{}",
        html! {
          head {
            title: "Hello, world!"
          }
        }
    )
}

pub fn document_suffix() -> String {
    format!(
        r#"</body>
</html>"#
    )
}
