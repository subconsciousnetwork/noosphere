//! Helpers for dealing with translation between the Noosphere and standard file
//! system representation of content
use anyhow::anyhow;
use std::str::FromStr;

use strum_macros::{AsRefStr, Display, EnumString};

/// https://www.iana.org/assignments/media-types/media-types.xhtml
/// https://www.rfc-editor.org/rfc/rfc6838#section-4.2
#[derive(Display, EnumString, AsRefStr)]
pub enum SuperType {
    /// https://www.rfc-editor.org/rfc/rfc6838#section-4.2.5
    Application,
    /// https://www.rfc-editor.org/rfc/rfc6838#section-4.2.3
    Audio,
    /// https://www.rfc-editor.org/rfc/rfc6838#section-4.2.2
    Image,
    /// https://www.iana.org/assignments/media-types/media-types.xhtml#message
    Message,
    /// https://www.rfc-editor.org/rfc/rfc6838#section-4.2.6
    Multipart,
    /// https://www.rfc-editor.org/rfc/rfc6838#section-4.2.1
    Text,
    /// https://www.rfc-editor.org/rfc/rfc6838#section-4.2.4
    Video,
    /// https://www.iana.org/assignments/media-types/media-types.xhtml#font
    Font,
    /// https://www.iana.org/assignments/media-types/media-types.xhtml#examples
    Example,
    /// https://www.iana.org/assignments/media-types/media-types.xhtml#model
    Model,
    /// https://www.ch.ic.ac.uk/chemime/
    Chemical,
    /// Everything else
    Unknown(String),
}

/// https://www.rfc-editor.org/rfc/rfc6838#section-3
#[derive(Display, EnumString, AsRefStr)]
pub enum Tree {
    /// https://www.rfc-editor.org/rfc/rfc6838#section-3.1
    Standard,
    /// https://www.rfc-editor.org/rfc/rfc6838#section-3.2
    #[strum(serialize = "vnd")]
    Vendor,
    /// https://www.rfc-editor.org/rfc/rfc6838#section-3.3
    #[strum(serialize = "prs")]
    Personal,
    /// https://www.rfc-editor.org/rfc/rfc6838#section-3.4
    #[strum(serialize = "x")]
    Unregistered,
    /// Everything else
    Unknown(String),
}

/// Internal state transitions for string parsing
enum MimeParseState {
    Type,
    Tree,
    Subtype,
    Suffix,
    Parameter,
}

/// A parsed representation of a media type
pub struct Mime {
    /// The top-level type, see [SuperType]
    pub super_type: SuperType,
    /// The registration tree, see [Tree]
    pub tree: Option<Tree>,
    /// The sub-type, see https://www.rfc-editor.org/rfc/rfc6838#section-4.2
    pub subtype: String,
    /// Optional suffixes, see https://www.rfc-editor.org/rfc/rfc6838#section-4.2.8
    pub suffix: Option<Vec<String>>,
    /// Optional trailing parameter, see https://www.rfc-editor.org/rfc/rfc6838#section-4.3
    pub parameter: Option<String>,
}

impl std::fmt::Display for Mime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let super_type = match &self.super_type {
            SuperType::Unknown(super_type) => super_type.as_str(),
            any_other => any_other.as_ref(),
        };

        let mut parts: Vec<&str> = vec![super_type, "/"];

        if let Some(tree) = &self.tree {
            match tree {
                Tree::Standard => (),
                Tree::Unknown(tree) => {
                    parts.push(tree.as_str());
                    parts.push(".");
                }
                any_other => {
                    parts.push(any_other.as_ref());
                    parts.push(".");
                }
            }
        }

        parts.push(self.subtype.as_ref());

        if let Some(suffix) = &self.suffix {
            for part in suffix {
                parts.push("+");
                parts.push(part.as_str());
            }
        }

        if let Some(parameter) = &self.parameter {
            parts.push(";");
            parts.push(parameter.as_str());
        }

        write!(f, "{}", parts.join(""))
    }
}

// mime-type = type "/" [tree "."] subtype ["+" suffix]* [";" parameter];
impl FromStr for Mime {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut state = MimeParseState::Type;

        state = MimeParseState::Tree;

        let mut super_type: Option<String> = None; //jString::new();
        let mut tree = None;
        let mut subtype = String::new();
        let mut suffix: Option<Vec<String>> = None;
        let mut parameter: Option<String> = None;

        for char in s.chars() {
            match state {
                MimeParseState::Type => match char {
                    '/' => state = MimeParseState::Tree,
                    _ => {
                        if let Some(super_type) = &mut super_type {
                            super_type.push(char);
                        } else {
                            super_type = Some(String::from(char));
                        }
                    }
                },
                MimeParseState::Tree => match char {
                    '.' => state = MimeParseState::Subtype,
                    '+' | ';' => return Err(anyhow!("Missing or misrepresented subtype")),
                    _ => {
                        if tree.is_none() {
                            tree = Some(String::new());
                        }

                        if let Some(tree) = &mut tree {
                            tree.push(char)
                        }
                    }
                },
                MimeParseState::Subtype => match char {
                    '+' => state = MimeParseState::Suffix,
                    ';' => state = MimeParseState::Parameter,
                    _ => subtype.push(char),
                },
                MimeParseState::Suffix => match char {
                    '+' => {
                        if let Some(suffix) = &mut suffix {
                            suffix.push(String::new());
                        } else {
                            suffix = Some(vec![String::new()]);
                        }
                    }
                    ';' => state = MimeParseState::Parameter,
                    _ => {
                        if let Some(suffix) = &mut suffix {
                            if let Some(element) = suffix.get_mut(0) {
                                element.push(char);
                            } else {
                                warn!("No initialized string in suffix");
                            }
                        } else {
                            suffix = Some(vec![String::from(char)])
                        }
                    }
                },
                MimeParseState::Parameter => {
                    if let Some(parameter) = &mut parameter {
                        parameter.push(char);
                    } else {
                        parameter = Some(String::from(char));
                    }
                }
            }
        }

        let tree = if let Some(tree) = tree {
            Some(Tree::from_str(&tree)?)
        } else {
            None
        };

        Ok(Mime {
            super_type: todo!(),
            tree,
            subtype,
            suffix,
            parameter,
        })
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use super::{Mime, SuperType, Tree};

    #[test]
    fn it_can_parse_mimes_from_strings() -> Result<()> {
        let test_cases = [
            (
                "text/plain",
                Mime {
                    super_type: SuperType::Text,
                    tree: None,
                    subtype: "plain".into(),
                    suffix: None,
                    parameter: None,
                },
            ),
            (
                "application/json",
                Mime {
                    super_type: SuperType::Application,
                    tree: None,
                    subtype: "json".into(),
                    suffix: None,
                    parameter: None,
                },
            ),
            (
                "application/vnd.subconscious+json",
                Mime {
                    super_type: SuperType::Application,
                    tree: Some(Tree::Vendor),
                    subtype: "subconscious".into(),
                    suffix: Some(vec!["json".into()]),
                    parameter: None,
                },
            ),
            "text/vnd.subconscious+subtext+text; hotsauce",
        ];

        Ok(())
    }
}
