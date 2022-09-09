use anyhow::anyhow;
use std::{fmt::Display, str::FromStr};

#[derive(Debug, PartialEq)]
pub enum Peer {
    Name(String),
    Did(String),
    None,
}

#[derive(Debug, PartialEq)]
pub enum Link {
    Slug(String),
    // TODO(subconsciousnetwork/subtext#36): Maybe support CIDs in slashlinks
    // Cid(Cid),
    None,
}

#[derive(Debug, PartialEq)]
pub struct Slashlink {
    pub peer: Peer,
    pub link: Link,
}

impl FromStr for Slashlink {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parsing_peer = false;
        let mut parsing_link = false;

        let mut raw_peer = String::new();
        let mut link = Link::None;

        for (index, character) in s.char_indices() {
            match character {
                '@' if index == 0 => {
                    parsing_peer = true;
                }
                '/' if index == 0 || parsing_peer => {
                    parsing_peer = false;
                    parsing_link = true;
                }
                _ if parsing_peer => raw_peer.push(character),
                _ if parsing_link => {
                    link = Link::Slug(s[index..].to_string());
                    break;
                }
                _ => {
                    break;
                }
            }
        }

        let peer = if raw_peer.len() > 0 {
            match raw_peer[0..4].to_lowercase().as_str() {
                "did:" => Peer::Did(raw_peer),
                _ => Peer::Name(raw_peer),
            }
        } else {
            Peer::None
        };

        if peer == Peer::None && link == Link::None {
            Err(anyhow!("Could not parse {} as SlashLink", s))
        } else {
            Ok(Slashlink { peer, link })
        }
    }
}

impl Display for Slashlink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match &self.peer {
            Peer::Name(name) => write!(f, "@{}", name),
            Peer::Did(did) => write!(f, "@{}", did),
            Peer::None => Ok(()),
        }?;

        match &self.link {
            Link::Slug(slug) => write!(f, "/{}", slug),
            Link::None => Ok(()),
        }?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use crate::slashlink::{Link, Peer, Slashlink};

    #[test]
    fn it_can_parse_a_basic_slashlink() {
        let slashlink = Slashlink::from_str("/foo-bar").unwrap();

        assert_eq!(slashlink.peer, Peer::None);
        assert_eq!(slashlink.link, Link::Slug("foo-bar".into()));
    }

    #[test]
    fn it_can_parse_a_basic_slashlink_with_a_peer_name() {
        let slashlink = Slashlink::from_str("@cdata/foo-bar").unwrap();

        assert_eq!(slashlink.peer, Peer::Name("cdata".into()));
        assert_eq!(slashlink.link, Link::Slug("foo-bar".into()));
    }

    #[test]
    #[ignore = "TODO(subconsciousnetwork/subtext#36)"]
    fn it_can_parse_a_slashlink_that_is_a_cid() {}

    #[test]
    fn it_can_parse_a_slashlink_with_a_peer_did() {
        let slashlink = Slashlink::from_str("@did:test:alice/foo-bar").unwrap();
        assert_eq!(slashlink.peer, Peer::Did("did:test:alice".into()));
        assert_eq!(slashlink.link, Link::Slug("foo-bar".into()));
    }

    #[test]
    fn it_can_parse_a_slashlink_with_only_a_peer() {
        let slashlink = Slashlink::from_str("@cdata").unwrap();
        assert_eq!(slashlink.peer, Peer::Name("cdata".into()));
    }

    #[test]
    fn it_will_not_parse_a_non_slashlink() {
        let non_slashlinks = vec!["cdata", "@", "/", "@/", "foo/bar"];
        for test_case in non_slashlinks {
            println!("Checking {}", test_case);
            assert!(Slashlink::from_str(test_case).is_err())
        }
    }
}
