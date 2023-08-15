//! Helpers for dealing with translation between the Noosphere and standard file
//! system representation of content. These constructs endeavor to be maximally
//! spec-compatible without sacrificing flexibility, but they have not been
//! vetted for correctness.
use anyhow::anyhow;
use std::str::FromStr;

use strum_macros::{AsRefStr, Display, EnumString};

/// https://www.iana.org/assignments/media-types/media-types.xhtml
/// https://www.rfc-editor.org/rfc/rfc6838#section-4.2
#[derive(Display, EnumString, AsRefStr, Clone, Debug, PartialEq, Eq)]
#[strum(serialize_all = "lowercase")]
pub enum Type {
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
#[derive(Display, EnumString, AsRefStr, Clone, Debug, PartialEq, Eq)]
#[strum(serialize_all = "lowercase")]
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

/// https://www.iana.org/assignments/media-type-structured-suffix/media-type-structured-suffix.xml
/// https://datatracker.ietf.org/doc/html/rfc3023.html
#[derive(Display, EnumString, AsRefStr, Clone, Debug, PartialEq, Eq)]
#[strum(serialize_all = "lowercase")]
pub enum Suffix {
    /// XML
    Xml,
    /// JSON
    Json,
    /// JSON Text Sequence
    #[strum(serialize = "json-seq")]
    JsonSeq,
    /// CBOR
    Cbor,
    /// CBOR Sequence
    #[strum(serialize = "cbor-seq")]
    CborSeq,
    /// Basic Encoding Rules
    Ber,
    /// Distinguished Encoding Rules
    Der,
    /// Fast Infoset Document
    FastInfoset,
    /// WAP Binary XML
    WbXml,
    /// ZIP
    Zip,
    /// gzip
    Gzip,
    /// Type Length Value
    Tlv,
    /// SQLite 3 Database
    Sqlite3,
    /// JSON Web Tokens
    Jwt,
    /// Zstandard
    Zstd,
    /// YAML
    Yaml,
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

#[derive(Clone, Debug, PartialEq, Eq)]
/// A parsed representation of a media type.
pub struct Mime {
    /// The top-level type, see [Type]; noting that this should be called
    /// 'type', but we can't call a field 'type' in Rust.
    pub top_level_type: Type,
    /// The registration tree, see [Tree]
    pub tree: Tree,
    /// The sub-type, see https://www.rfc-editor.org/rfc/rfc6838#section-4.2
    pub subtype: String,
    /// Optional suffixes, see https://www.rfc-editor.org/rfc/rfc6838#section-4.2.8
    pub suffix: Option<Vec<Suffix>>,
    /// Optional trailing parameter, see https://www.rfc-editor.org/rfc/rfc6838#section-4.3
    pub parameter: Option<String>,
}

impl std::fmt::Display for Mime {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let top_level_type = match &self.top_level_type {
            Type::Unknown(top_level_type) => top_level_type.as_str(),
            any_other => any_other.as_ref(),
        };

        let mut parts: Vec<&str> = vec![top_level_type, "/"];

        match &self.tree {
            Tree::Standard => (),
            Tree::Unknown(tree) => {
                parts.push(tree.as_str());
                parts.push(".");
            }
            any_other => {
                parts.push(any_other.as_ref());
                parts.push(".");
            }
        };

        parts.push(self.subtype.as_ref());

        if let Some(suffix) = &self.suffix {
            for part in suffix {
                match part {
                    Suffix::Unknown(suffix) => {
                        parts.push("+");
                        parts.push(suffix.as_str());
                    }
                    any_other => {
                        parts.push("+");
                        parts.push(any_other.as_ref());
                    }
                }
            }
        }

        if let Some(parameter) = &self.parameter {
            parts.push(";");
            parts.push(parameter.as_str());
        }

        write!(f, "{}", parts.join(""))
    }
}

impl FromStr for Mime {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // mime-type = type "/" [tree "."] subtype ["+" suffix]* [";" parameter];
        //
        // NOTE: This is a simplification of the grammar described in
        // https://datatracker.ietf.org/doc/html/rfc2045#section-2 as documented
        // in https://en.m.wikipedia.org/wiki/Media_type#Naming
        let mut state = MimeParseState::Type;

        let mut raw_top_level_type = String::new();
        let mut tree = String::new();
        let mut subtype = None;
        let mut suffix: Option<Vec<String>> = None;
        let mut parameter: Option<String> = None;

        let mut top_level_type = None;

        for char in s.chars() {
            match state {
                MimeParseState::Type => match char {
                    '/' => {
                        top_level_type =
                            Some(raw_top_level_type.parse::<Type>().unwrap_or_else(|_| {
                                Type::Unknown(std::mem::take(&mut raw_top_level_type))
                            }));
                        state = MimeParseState::Tree;
                    }
                    _ => {
                        raw_top_level_type.push(char);
                    }
                },
                MimeParseState::Tree => match char {
                    '.' => state = MimeParseState::Subtype,
                    '+' => state = MimeParseState::Suffix,
                    ';' => state = MimeParseState::Parameter,
                    _ => {
                        // NOTE: We support parsing strings that use the legacy
                        // `x-` prefix for the unregistered tree, but
                        // serializing a [Mime] always produces a string with
                        // the preferred `x.` prefix (so in this case, the round
                        // trip is lossy).
                        //
                        // Strictly speaking, `x-` prefix is no longer
                        // considered part of the "unregistered" tree, but for
                        // our interpretation of mimes it may as well be. For
                        // example, if we are trying to infer the file extension
                        // for a content type, the type `text/x-markdown` should
                        // get the same extension as `text/x.markdown`, which
                        // should get the same extension as `text/markdown`. The
                        // virtue in parsing as a [Mime] arises from separating
                        // the tree prefix from the subtype.
                        //
                        // https://en.m.wikipedia.org/wiki/Media_type#Unregistered_tree
                        if char == '-' {
                            if tree.len() == 1 && "x" == &tree.to_lowercase() {
                                state = MimeParseState::Subtype;
                                continue;
                            }
                        }

                        tree.push(char);
                    }
                },
                MimeParseState::Subtype => match char {
                    '+' => state = MimeParseState::Suffix,
                    ';' => state = MimeParseState::Parameter,
                    _ => {
                        if subtype.is_none() {
                            subtype = Some(String::new());
                        }

                        if let Some(subtype) = &mut subtype {
                            subtype.push(char);
                        }
                    }
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
                            if let Some(element) = suffix.last_mut() {
                                element.push(char);
                            } else {
                                warn!("No initialized string in suffix");
                            }
                        } else {
                            suffix = Some(vec![String::from(char)]);
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

        let top_level_type = top_level_type.ok_or_else(|| anyhow!("Missing top-level type"))?;

        let (tree, subtype) = if let Some(subtype) = subtype {
            (
                tree.parse::<Tree>().unwrap_or_else(|_| Tree::Unknown(tree)),
                subtype,
            )
        } else {
            (Tree::Standard, tree)
        };

        let suffix = suffix.map(|suffix| {
            suffix
                .into_iter()
                .map(|suffix| {
                    suffix
                        .parse::<Suffix>()
                        .unwrap_or_else(|_| Suffix::Unknown(suffix))
                })
                .collect()
        });

        Ok(Mime {
            top_level_type,
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
    use noosphere_core::tracing::initialize_tracing;

    use crate::mime::Suffix;

    use super::{Mime, Tree, Type};

    fn test_cases() -> Vec<(&'static str, Mime, Option<&'static str>)> {
        vec![
            (
                "text/plain",
                Mime {
                    top_level_type: Type::Text,
                    tree: Tree::Standard,
                    subtype: "plain".into(),
                    suffix: None,
                    parameter: None,
                },
                None,
            ),
            (
                "text/plain; special+kind; foo",
                Mime {
                    top_level_type: Type::Text,
                    tree: Tree::Standard,
                    subtype: "plain".into(),
                    suffix: None,
                    parameter: Some(" special+kind; foo".into()),
                },
                None,
            ),
            (
                "random/numbers",
                Mime {
                    top_level_type: Type::Unknown("random".into()),
                    tree: Tree::Standard,
                    subtype: "numbers".into(),
                    suffix: None,
                    parameter: None,
                },
                None,
            ),
            (
                "text/x-markdown",
                Mime {
                    top_level_type: Type::Text,
                    tree: Tree::Unregistered,
                    subtype: "markdown".into(),
                    suffix: None,
                    parameter: None,
                },
                Some("text/x.markdown"),
            ),
            (
                "text/x.markdown",
                Mime {
                    top_level_type: Type::Text,
                    tree: Tree::Unregistered,
                    subtype: "markdown".into(),
                    suffix: None,
                    parameter: None,
                },
                None,
            ),
            (
                "application/json",
                Mime {
                    top_level_type: Type::Application,
                    tree: Tree::Standard,
                    subtype: "json".into(),
                    suffix: None,
                    parameter: None,
                },
                None,
            ),
            (
                "application/json+suffix",
                Mime {
                    top_level_type: Type::Application,
                    tree: Tree::Standard,
                    subtype: "json".into(),
                    suffix: Some(vec![Suffix::Unknown("suffix".into())]),
                    parameter: None,
                },
                None,
            ),
            (
                "application/vnd.subconscious+json",
                Mime {
                    top_level_type: Type::Application,
                    tree: Tree::Vendor,
                    subtype: "subconscious".into(),
                    suffix: Some(vec![Suffix::Json]),
                    parameter: None,
                },
                None,
            ),
            (
                "text/vnd.subconscious.noosphere+subtext+cbor; hotsauce",
                Mime {
                    top_level_type: Type::Text,
                    tree: Tree::Vendor,
                    subtype: "subconscious.noosphere".into(),
                    suffix: Some(vec![Suffix::Unknown("subtext".into()), Suffix::Cbor]),
                    parameter: Some(" hotsauce".into()),
                },
                None,
            ),
        ]
    }

    #[test]
    fn it_can_parse_strings_as_mimes() -> Result<()> {
        initialize_tracing(None);

        for (test_string, expected, _) in test_cases() {
            assert_eq!(test_string.parse::<Mime>()?, expected);
        }

        Ok(())
    }

    #[test]
    fn it_can_serialize_mimes_as_strings() -> Result<()> {
        initialize_tracing(None);

        for (expected, mime, serialized) in test_cases() {
            assert_eq!(&mime.to_string(), serialized.unwrap_or(expected));
        }

        Ok(())
    }
}
