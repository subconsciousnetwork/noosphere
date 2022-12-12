#[derive(Clone, Debug)]
pub struct TextTransclude {
    pub title: Option<String>,
    pub excerpt: Option<String>,
    pub link_text: String,
    pub href: String,
}

/// The set of possible transcludes that may need to be rendered to a target
/// format. At this time, only text transcludes are supported.
#[derive(Clone, Debug)]
pub enum Transclude {
    // TODO
    // Rich,
    // Interactive,
    // Bitmap,
    Text(TextTransclude),
}
