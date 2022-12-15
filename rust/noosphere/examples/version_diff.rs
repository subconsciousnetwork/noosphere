use std::fmt::Display;

use clap::Parser;
use semver::Version;

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    #[arg(short, long)]
    left: Version,
    #[arg(short, long)]
    right: Version,
}

#[derive(PartialEq, Debug)]
pub enum VersionDiff {
    Breaking,
    Feature,
    Fix,
    None,
    Invalid,
}

fn version_diff(left: &Version, right: &Version) -> VersionDiff {
    if left.pre != right.pre {
        VersionDiff::Breaking
    } else if left.major == right.major {
        if left.minor == right.minor {
            if left.patch == right.patch {
                VersionDiff::None
            } else if left.patch < right.patch {
                if left.major == 0 {
                    if left.minor == 0 {
                        VersionDiff::Breaking
                    } else {
                        VersionDiff::Feature
                    }
                } else {
                    VersionDiff::Fix
                }
            } else {
                VersionDiff::Invalid
            }
        } else if left.minor < right.minor {
            if left.major == 0 {
                VersionDiff::Breaking
            } else {
                VersionDiff::Feature
            }
        } else {
            VersionDiff::Invalid
        }
    } else if left.major < right.major {
        VersionDiff::Breaking
    } else {
        VersionDiff::Invalid
    }
}

impl Display for VersionDiff {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                VersionDiff::Breaking => "feat!:",
                VersionDiff::Feature => "feat:",
                VersionDiff::Fix => "fix:",
                VersionDiff::None => "chore:",
                VersionDiff::Invalid => "ERROR",
            }
        )
    }
}

fn main() {
    let Args { left, right } = Args::parse();

    let diff = version_diff(&left, &right);

    println!("{}", diff);
}

#[cfg(test)]
mod tests {
    use semver::Version;

    use super::{version_diff, VersionDiff};

    fn parse_and_diff(left: &str, right: &str) -> VersionDiff {
        version_diff(
            &Version::parse(left).unwrap(),
            &Version::parse(right).unwrap(),
        )
    }

    #[test]
    pub fn it_diffs_versions() {
        let test_cases = vec![
            ("0.0.1", "0.0.1", VersionDiff::None),
            ("0.1.0", "0.1.0", VersionDiff::None),
            ("1.0.0", "1.0.0", VersionDiff::None),
            ("0.0.1", "0.0.2", VersionDiff::Breaking),
            ("0.1.0", "0.2.0", VersionDiff::Breaking),
            ("1.1.2", "2.0.0", VersionDiff::Breaking),
            ("1.0.0-alpha.1", "1.0.0-alpha.2", VersionDiff::Breaking),
            ("0.1.0", "0.1.1", VersionDiff::Feature),
            ("1.1.0", "1.2.0", VersionDiff::Feature),
            ("1.1.0", "1.2.0", VersionDiff::Feature),
            ("1.0.0", "1.0.1", VersionDiff::Fix),
            ("1.1.0", "1.1.1", VersionDiff::Fix),
        ];

        for (left, right, expected_diff) in test_cases {
            assert_eq!(
                parse_and_diff(left, right),
                expected_diff,
                "Left: {}, Right: {}",
                left,
                right
            );
        }
    }
}
