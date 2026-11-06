//! The TOML catalog of apps and actions.

use std::{
    collections::HashSet,
    fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Deserializer};

use crate::{Error, Family};

/// Native package names per family, plus cross-distro fallbacks.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Packages {
    #[serde(default, deserialize_with = "one_or_many")]
    pub fedora: Vec<String>,
    #[serde(default, deserialize_with = "one_or_many")]
    pub debian: Vec<String>,
    #[serde(default, deserialize_with = "one_or_many")]
    pub arch: Vec<String>,
    /// Arch User Repository packages (used only when an AUR helper exists).
    #[serde(default, deserialize_with = "one_or_many")]
    pub aur: Vec<String>,
    #[serde(default, deserialize_with = "one_or_many")]
    pub flatpak: Vec<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct App {
    pub id: String,
    pub name: String,
    pub category: String,
    #[serde(default)]
    pub description: String,
    pub packages: Packages,
}

/// How an app will be installed on a given system.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InstallMethod {
    Native(Vec<String>),
    Aur(Vec<String>),
    Flatpak(Vec<String>),
}

impl App {
    /// Preferred install method: native, then AUR (Arch with helper), then Flatpak.
    pub fn install_method(&self, family: Family, has_aur: bool) -> Option<InstallMethod> {
        let p = &self.packages;
        let native = match family {
            Family::Fedora => &p.fedora,
            Family::Debian => &p.debian,
            Family::Arch => &p.arch,
        };
        if !native.is_empty() {
            Some(InstallMethod::Native(native.clone()))
        } else if family == Family::Arch && has_aur && !p.aur.is_empty() {
            Some(InstallMethod::Aur(p.aur.clone()))
        } else if !p.flatpak.is_empty() {
            Some(InstallMethod::Flatpak(p.flatpak.clone()))
        } else {
            None
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ActionKind {
    Tweak,
    Maintenance,
    Setup,
}

impl ActionKind {
    pub const ALL: [ActionKind; 3] = [
        ActionKind::Tweak,
        ActionKind::Maintenance,
        ActionKind::Setup,
    ];

    pub fn file_name(self) -> &'static str {
        match self {
            ActionKind::Tweak => "tweaks.toml",
            ActionKind::Maintenance => "maintenance.toml",
            ActionKind::Setup => "setup.toml",
        }
    }

    pub fn title(self) -> &'static str {
        match self {
            ActionKind::Tweak => "Tweaks",
            ActionKind::Maintenance => "Maintenance",
            ActionKind::Setup => "Setup",
        }
    }
}

/// A script-backed action (tweak, maintenance task, or setup bundle).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Action {
    pub id: String,
    pub name: String,
    #[serde(default = "default_category")]
    pub category: String,
    #[serde(default)]
    pub description: String,
    /// Families this applies to; empty = all.
    #[serde(default)]
    pub distros: Vec<Family>,
    /// Run as root.
    #[serde(default)]
    pub root: bool,
    /// `script/path.sh [args...]`, relative to the scripts dir.
    pub run: String,
    #[serde(default)]
    pub revert: Option<String>,
    /// Exit 0 means "already applied". Runs unprivileged.
    #[serde(default)]
    pub check: Option<String>,
    /// Extra warning shown before running.
    #[serde(default)]
    pub confirm: Option<String>,
    #[serde(skip, default = "default_kind")]
    pub kind: ActionKind,
}

fn default_category() -> String {
    "General".into()
}
fn default_kind() -> ActionKind {
    ActionKind::Tweak
}

impl Action {
    pub fn applies_to(&self, family: Family) -> bool {
        self.distros.is_empty() || self.distros.contains(&family)
    }

    /// All script invocations this action references.
    pub fn invocations(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.run.as_str())
            .chain(self.revert.as_deref())
            .chain(self.check.as_deref())
    }
}

/// Split an invocation like `tweaks/foo.sh apply` into (script, args).
pub fn split_invocation(inv: &str) -> (&str, Vec<&str>) {
    let mut parts = inv.split_whitespace();
    let script = parts.next().unwrap_or("");
    (script, parts.collect())
}

#[derive(Debug, Clone, Default)]
pub struct Catalog {
    pub apps: Vec<App>,
    /// Tweaks, then maintenance, then setup, each in file order.
    pub actions: Vec<Action>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AppsFile {
    #[serde(default, rename = "app")]
    apps: Vec<App>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ActionsFile {
    #[serde(default, rename = "action")]
    actions: Vec<Action>,
}

impl Catalog {
    /// Load `apps.toml`, `tweaks.toml`, `maintenance.toml`, `setup.toml` from `dir`.
    /// Missing files are treated as empty.
    pub fn load(dir: &Path) -> Result<Self, Error> {
        let apps = read_toml::<AppsFile>(&dir.join("apps.toml"))?
            .map(|f| f.apps)
            .unwrap_or_default();
        let mut actions = Vec::new();
        for kind in ActionKind::ALL {
            if let Some(f) = read_toml::<ActionsFile>(&dir.join(kind.file_name()))? {
                actions.extend(f.actions.into_iter().map(|mut a| {
                    a.kind = kind;
                    a
                }));
            }
        }
        Ok(Catalog { apps, actions })
    }

    /// Only entries usable on `family`.
    pub fn for_family(&self, family: Family, has_aur: bool) -> Catalog {
        Catalog {
            apps: self
                .apps
                .iter()
                .filter(|a| a.install_method(family, has_aur).is_some())
                .cloned()
                .collect(),
            actions: self
                .actions
                .iter()
                .filter(|a| a.applies_to(family))
                .cloned()
                .collect(),
        }
    }

    pub fn app(&self, id: &str) -> Option<&App> {
        self.apps.iter().find(|a| a.id == id)
    }

    pub fn action(&self, id: &str) -> Option<&Action> {
        self.actions.iter().find(|a| a.id == id)
    }

    pub fn actions_of(&self, kind: ActionKind) -> impl Iterator<Item = &Action> {
        self.actions.iter().filter(move |a| a.kind == kind)
    }

    /// Consistency problems: duplicate ids, missing scripts, uninstallable apps.
    pub fn validate(&self, scripts_dir: &Path) -> Vec<String> {
        let mut problems = Vec::new();
        let mut seen = HashSet::new();
        let ids = self
            .apps
            .iter()
            .map(|a| &a.id)
            .chain(self.actions.iter().map(|a| &a.id));
        for id in ids {
            if !seen.insert(id) {
                problems.push(format!("duplicate id `{id}`"));
            }
            if id.is_empty()
                || id
                    .chars()
                    .any(|c| !(c.is_ascii_alphanumeric() || c == '-' || c == '_'))
            {
                problems.push(format!("invalid id `{id}` (use [a-z0-9_-])"));
            }
        }
        for app in &self.apps {
            let installable = Family::ALL
                .iter()
                .any(|f| app.install_method(*f, true).is_some());
            if !installable {
                problems.push(format!("app `{}` has no packages for any distro", app.id));
            }
        }
        for action in &self.actions {
            for inv in action.invocations() {
                let (script, _) = split_invocation(inv);
                let path: PathBuf = scripts_dir.join(script);
                if script.is_empty() || script.contains("..") || !path.is_file() {
                    problems.push(format!(
                        "action `{}` references missing script `{script}`",
                        action.id
                    ));
                }
            }
        }
        problems
    }
}

fn read_toml<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<Option<T>, Error> {
    if !path.exists() {
        return Ok(None);
    }
    let text = fs::read_to_string(path)?;
    toml::from_str(&text).map(Some).map_err(|e| Error::Catalog {
        file: path.display().to_string(),
        message: e.to_string(),
    })
}

fn one_or_many<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<String>, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum OneOrMany {
        One(String),
        Many(Vec<String>),
    }
    Ok(match OneOrMany::deserialize(d)? {
        OneOrMany::One(s) => vec![s],
        OneOrMany::Many(v) => v,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn app(toml_src: &str) -> App {
        toml::from_str(toml_src).unwrap()
    }

    #[test]
    fn install_method_preference() {
        let a = app(r#"
            id = "x"
            name = "X"
            category = "C"
            [packages]
            fedora = "x"
            aur = ["x-bin"]
            flatpak = "org.x.X"
        "#);
        assert_eq!(
            a.install_method(Family::Fedora, false),
            Some(InstallMethod::Native(vec!["x".into()]))
        );
        assert_eq!(
            a.install_method(Family::Arch, true),
            Some(InstallMethod::Aur(vec!["x-bin".into()]))
        );
        assert_eq!(
            a.install_method(Family::Arch, false),
            Some(InstallMethod::Flatpak(vec!["org.x.X".into()]))
        );
        assert_eq!(
            a.install_method(Family::Debian, false),
            Some(InstallMethod::Flatpak(vec!["org.x.X".into()]))
        );
    }

    #[test]
    fn no_method() {
        let a = app(r#"
            id = "x"
            name = "X"
            category = "C"
            [packages]
            fedora = "x"
        "#);
        assert_eq!(a.install_method(Family::Debian, true), None);
    }

    #[test]
    fn split() {
        assert_eq!(
            split_invocation("tweaks/a.sh apply now"),
            ("tweaks/a.sh", vec!["apply", "now"])
        );
    }
}
