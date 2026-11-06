//! Linux distribution detection from `os-release`.

use std::{fmt, fs, path::Path, str::FromStr};

use serde::Deserialize;

use crate::Error;

/// The distro families nxtoolkit knows how to drive.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "lowercase")]
pub enum Family {
    Fedora,
    Debian,
    Arch,
}

impl Family {
    pub const ALL: [Family; 3] = [Family::Fedora, Family::Debian, Family::Arch];

    pub fn as_str(self) -> &'static str {
        match self {
            Family::Fedora => "fedora",
            Family::Debian => "debian",
            Family::Arch => "arch",
        }
    }

    /// Map an os-release `ID` / `ID_LIKE` token to a family.
    fn from_os_id(id: &str) -> Option<Self> {
        match id {
            "fedora" | "rhel" | "centos" | "rocky" | "almalinux" | "nobara" | "ultramarine" => {
                Some(Family::Fedora)
            }
            "debian" | "ubuntu" | "linuxmint" | "pop" | "elementary" | "zorin" | "kali"
            | "raspbian" => Some(Family::Debian),
            "arch" | "archlinux" | "endeavouros" | "manjaro" | "cachyos" | "garuda" => {
                Some(Family::Arch)
            }
            _ => None,
        }
    }
}

impl fmt::Display for Family {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Family {
    type Err = Error;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Family::from_os_id(&s.to_ascii_lowercase())
            .ok_or_else(|| Error::UnsupportedDistro(s.to_string()))
    }
}

/// A detected distribution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Distro {
    pub id: String,
    pub id_like: Vec<String>,
    pub name: String,
    pub version_id: Option<String>,
    pub family: Family,
}

impl Distro {
    /// Parse the contents of an `os-release` file.
    pub fn parse(content: &str) -> Result<Self, Error> {
        let mut id = None;
        let mut id_like = Vec::new();
        let mut name = None;
        let mut pretty = None;
        let mut version_id = None;
        for line in content.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let value = unquote(value);
            match key {
                "ID" => id = Some(value.to_ascii_lowercase()),
                "ID_LIKE" => {
                    id_like = value
                        .split_whitespace()
                        .map(|s| s.to_ascii_lowercase())
                        .collect()
                }
                "NAME" => name = Some(value),
                "PRETTY_NAME" => pretty = Some(value),
                "VERSION_ID" => version_id = Some(value),
                _ => {}
            }
        }
        let id = id.ok_or_else(|| Error::UnsupportedDistro("os-release has no ID".into()))?;
        let family = std::iter::once(&id)
            .chain(id_like.iter())
            .find_map(|s| Family::from_os_id(s))
            .ok_or_else(|| Error::UnsupportedDistro(id.clone()))?;
        Ok(Distro {
            name: pretty.or(name).unwrap_or_else(|| id.clone()),
            id,
            id_like,
            version_id,
            family,
        })
    }

    /// Detect the running distribution.
    pub fn detect() -> Result<Self, Error> {
        for path in ["/etc/os-release", "/usr/lib/os-release"] {
            if Path::new(path).exists() {
                let content = fs::read_to_string(path)?;
                return Distro::parse(&content);
            }
        }
        Err(Error::UnsupportedDistro("no os-release file found".into()))
    }

    /// A synthetic distro for a family (used for `--distro` overrides / dry runs).
    pub fn synthetic(family: Family) -> Self {
        Distro {
            id: family.as_str().into(),
            id_like: Vec::new(),
            name: format!("{family} (simulated)"),
            version_id: None,
            family,
        }
    }
}

fn unquote(v: &str) -> String {
    let v = v.trim();
    let inner = v
        .strip_prefix('"')
        .and_then(|s| s.strip_suffix('"'))
        .or_else(|| v.strip_prefix('\'').and_then(|s| s.strip_suffix('\'')))
        .unwrap_or(v);
    inner.replace("\\\"", "\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURES: &[(&str, &str, Family)] = &[
        (
            "fedora45",
            include_str!("../tests/fixtures/os-release-fedora45"),
            Family::Fedora,
        ),
        (
            "ubuntu2404",
            include_str!("../tests/fixtures/os-release-ubuntu2404"),
            Family::Debian,
        ),
        (
            "debian13",
            include_str!("../tests/fixtures/os-release-debian13"),
            Family::Debian,
        ),
        (
            "arch",
            include_str!("../tests/fixtures/os-release-arch"),
            Family::Arch,
        ),
        (
            "mint",
            include_str!("../tests/fixtures/os-release-mint"),
            Family::Debian,
        ),
        (
            "endeavour",
            include_str!("../tests/fixtures/os-release-endeavouros"),
            Family::Arch,
        ),
    ];

    #[test]
    fn parses_fixtures() {
        for (name, content, family) in FIXTURES {
            let d = Distro::parse(content).unwrap_or_else(|e| panic!("{name}: {e}"));
            assert_eq!(d.family, *family, "{name}");
            assert!(!d.name.is_empty());
        }
    }

    #[test]
    fn fedora_details() {
        let d = Distro::parse(FIXTURES[0].1).unwrap();
        assert_eq!(d.id, "fedora");
        assert_eq!(d.version_id.as_deref(), Some("45"));
    }

    #[test]
    fn unknown_distro_errors() {
        assert!(Distro::parse("ID=gentoo\nNAME=Gentoo\n").is_err());
        assert!(Distro::parse("NAME=nothing\n").is_err());
    }

    #[test]
    fn family_from_str() {
        assert_eq!("Ubuntu".parse::<Family>().unwrap(), Family::Debian);
        assert!("gentoo".parse::<Family>().is_err());
    }
}
