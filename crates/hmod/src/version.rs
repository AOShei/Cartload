//! The `var/version` file: shell assignments naming the boot, kernel and script versions.

use std::{collections::BTreeMap, fmt};

/// Name of the variable holding the hakchi script version, the package's identity.
pub const HAKCHI_VERSION: &str = "hakchiVersion";
/// Name of the variable holding the boot (u-boot and boot image layout) version.
pub const BOOT_VERSION: &str = "bootVersion";
/// Name of the variable holding the kernel version.
pub const KERNEL_VERSION: &str = "kernelVersion";
/// Name of the variable holding the version of Cartload's system, in Cartload's own packages.
pub const CARTLOAD_VERSION: &str = "cartloadVersion";

/// The versions declared by a kernel package in `var/version`.
///
/// The file is a shell fragment sourced on the console, for example:
///
/// ```text
/// bootVersion='1.0.3'
/// hakchiVersion='v1.0.4-126'
/// kernelVersion='3.4.113.29-madmonkey'
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Version {
    /// `hakchiVersion`, e.g. `v1.0.4-126`. Always present.
    pub hakchi: String,
    /// `bootVersion`, e.g. `1.0.3`.
    pub boot: Option<String>,
    /// `kernelVersion`, e.g. `3.4.113.29-madmonkey`.
    pub kernel: Option<String>,
    /// `cartloadVersion`, e.g. `0.1.0`: present in Cartload's own system packages only.
    pub cartload: Option<String>,
    /// Every variable in the file, including the four above.
    pub variables: BTreeMap<String, String>,
}

impl Version {
    /// Parse the text of `var/version`. Returns `None` when it has no `hakchiVersion`.
    ///
    /// Accepts `name=value`, `name='value'` and `name="value"` lines (with `\"` and `\\`
    /// unescaped inside double quotes), an optional `export` prefix, blank lines and `#`
    /// comments. Anything else is ignored.
    pub fn parse(text: &str) -> Option<Self> {
        let variables = parse_assignments(text);
        let hakchi = variables.get(HAKCHI_VERSION)?.clone();
        Some(Self {
            hakchi,
            boot: variables.get(BOOT_VERSION).cloned(),
            kernel: variables.get(KERNEL_VERSION).cloned(),
            cartload: variables.get(CARTLOAD_VERSION).cloned(),
            variables,
        })
    }
}

impl fmt::Display for Version {
    /// `v1.0.4-126 (boot 1.0.3, kernel 3.4.113.29-madmonkey)`, omitting what is unknown;
    /// Cartload's own packages read `Cartload system 0.1.0 (hakchi v1.0.4-126, boot ...)`.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(cartload) = &self.cartload {
            write!(f, "Cartload system {cartload} (hakchi {}", self.hakchi)?;
            for (label, value) in [("boot", &self.boot), ("kernel", &self.kernel)] {
                if let Some(value) = value {
                    write!(f, ", {label} {value}")?;
                }
            }
            return f.write_str(")");
        }
        f.write_str(&self.hakchi)?;
        let details: Vec<String> = [("boot", &self.boot), ("kernel", &self.kernel)]
            .into_iter()
            .filter_map(|(label, value)| value.as_ref().map(|v| format!("{label} {v}")))
            .collect();
        if !details.is_empty() {
            write!(f, " ({})", details.join(", "))?;
        }
        Ok(())
    }
}

/// Parse `name=value` shell assignments, one per line. Later assignments win.
fn parse_assignments(text: &str) -> BTreeMap<String, String> {
    let mut variables = BTreeMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let line = line.strip_prefix("export ").map_or(line, str::trim_start);
        let Some((name, value)) = line.split_once('=') else {
            continue;
        };
        let name = name.trim_end();
        if !is_identifier(name) {
            continue;
        }
        variables.insert(name.to_string(), unquote(value.trim()));
    }
    variables
}

fn is_identifier(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || c == '_')
}

/// Strip one layer of shell quoting from a value.
fn unquote(value: &str) -> String {
    if let Some(inner) = value
        .strip_prefix('\'')
        .and_then(|rest| rest.strip_suffix('\''))
    {
        return inner.to_string();
    }
    if let Some(inner) = value
        .strip_prefix('"')
        .and_then(|rest| rest.strip_suffix('"'))
    {
        let mut out = String::with_capacity(inner.len());
        let mut chars = inner.chars();
        while let Some(c) = chars.next() {
            if c == '\\' {
                match chars.next() {
                    Some(escaped @ ('"' | '\\' | '$' | '`')) => out.push(escaped),
                    Some(other) => {
                        out.push('\\');
                        out.push(other);
                    }
                    None => out.push('\\'),
                }
            } else {
                out.push(c);
            }
        }
        return out;
    }
    // Bare word: ends at whitespace or a comment.
    value
        .split(|c: char| c.is_whitespace() || c == '#')
        .next()
        .unwrap_or("")
        .to_string()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used)]

    use super::*;
    use crate::test_support::VERSION_TEXT;

    #[test]
    fn parses_the_real_version_file() {
        let version = Version::parse(VERSION_TEXT).unwrap();
        assert_eq!(version.hakchi, "v1.0.4-126");
        assert_eq!(version.boot.as_deref(), Some("1.0.3"));
        assert_eq!(version.kernel.as_deref(), Some("3.4.113.29-madmonkey"));
        assert_eq!(version.variables.len(), 3);
        assert_eq!(
            version.to_string(),
            "v1.0.4-126 (boot 1.0.3, kernel 3.4.113.29-madmonkey)"
        );
    }

    #[test]
    fn accepts_other_quoting_styles_and_ignores_noise() {
        let text = "# comment\n\nexport hakchiVersion=\"v1.0.5 \\\"beta\\\"\"\nbootVersion=1.0.4 # trailing\n\
                    kernelVersion = '3.4'\n9bad='x'\nnot an assignment\nextra=\"a\\$b\"\n";
        let version = Version::parse(text).unwrap();
        assert_eq!(version.hakchi, "v1.0.5 \"beta\"");
        assert_eq!(version.boot.as_deref(), Some("1.0.4"));
        assert_eq!(version.kernel.as_deref(), Some("3.4"));
        assert_eq!(version.variables.get("extra").unwrap(), "a$b");
        assert!(!version.variables.contains_key("9bad"));
        assert_eq!(version.variables.len(), 4);
    }

    #[test]
    fn later_assignments_win() {
        let version = Version::parse("hakchiVersion='a'\nhakchiVersion='b'\n").unwrap();
        assert_eq!(version.hakchi, "b");
    }

    #[test]
    fn requires_hakchi_version() {
        assert!(Version::parse("bootVersion='1.0.3'\n").is_none());
        assert!(Version::parse("").is_none());
    }

    #[test]
    fn cartload_system_version() {
        let version = Version::parse(
            "bootVersion='1.0.3'\nhakchiVersion='v1.0.4-126'\nkernelVersion='3.4.113.30-cartload'\ncartloadVersion='0.1.0'\n",
        )
        .unwrap();
        assert_eq!(version.cartload.as_deref(), Some("0.1.0"));
        assert_eq!(
            version.to_string(),
            "Cartload system 0.1.0 (hakchi v1.0.4-126, boot 1.0.3, kernel 3.4.113.30-cartload)"
        );
        let hakchi = Version::parse("hakchiVersion='v1.0.4-126'\n").unwrap();
        assert_eq!(hakchi.cartload, None);
    }

    #[test]
    fn display_omits_unknown_parts() {
        let version = Version {
            hakchi: "v1".to_string(),
            boot: None,
            kernel: Some("3.4".to_string()),
            cartload: None,
            variables: BTreeMap::new(),
        };
        assert_eq!(version.to_string(), "v1 (kernel 3.4)");
        let bare = Version {
            kernel: None,
            ..version
        };
        assert_eq!(bare.to_string(), "v1");
    }

    #[test]
    fn unquote_handles_edge_cases() {
        assert_eq!(unquote("'"), "'");
        assert_eq!(unquote("\"unterminated"), "\"unterminated");
        assert_eq!(unquote("\"a\\\"b\""), "a\"b");
        assert_eq!(unquote("\"back\\\\slash\""), "back\\slash");
        assert_eq!(unquote("\"keep\\n\""), "keep\\n");
        assert_eq!(unquote("\"dangling\\\""), "dangling\\");
        assert_eq!(unquote(""), "");
    }
}
