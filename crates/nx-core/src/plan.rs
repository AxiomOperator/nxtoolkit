//! Turn a user selection into an ordered list of shell steps.

use std::{collections::HashSet, fmt::Write};

use crate::{
    Error, Family,
    catalog::{Action, ActionKind, Catalog, InstallMethod, split_invocation},
    pkg::{self, AurHelper, Flatpak, PackageManager},
    shell,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    /// Install apps / apply actions.
    Apply,
    /// Uninstall apps / revert actions.
    Revert,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Step {
    pub label: String,
    pub root: bool,
    /// A bash snippet. Runs under `set -euo pipefail` in a subshell.
    pub command: String,
}

#[derive(Debug, Clone, Default)]
pub struct Plan {
    pub steps: Vec<Step>,
    /// Things that were skipped or may surprise the user.
    pub warnings: Vec<String>,
    /// Per-action confirmation messages to show before running.
    pub confirmations: Vec<String>,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    pub fn needs_root(&self) -> bool {
        self.steps.iter().any(|s| s.root)
    }

    /// Human-readable dry-run output.
    pub fn render(&self) -> String {
        let mut out = String::new();
        for (i, s) in self.steps.iter().enumerate() {
            let who = if s.root { "root" } else { "user" };
            let _ = writeln!(out, "[{}] {} ({who})", i + 1, s.label);
            for line in s.command.lines() {
                let _ = writeln!(out, "    $ {line}");
            }
        }
        for c in &self.confirmations {
            let _ = writeln!(out, "! {c}");
        }
        for w in &self.warnings {
            let _ = writeln!(out, "warning: {w}");
        }
        out
    }
}

pub fn build(
    catalog: &Catalog,
    family: Family,
    aur: Option<&AurHelper>,
    ids: &[String],
    mode: Mode,
) -> Result<Plan, Error> {
    let mut app_ids = HashSet::new();
    let mut action_ids = HashSet::new();
    for id in ids {
        if catalog.app(id).is_some() {
            app_ids.insert(id.as_str());
        } else if catalog.action(id).is_some() {
            action_ids.insert(id.as_str());
        } else {
            return Err(Error::UnknownId(id.clone()));
        }
    }

    let mut plan = Plan::default();
    let actions_of = |kind: ActionKind| -> Vec<&Action> {
        catalog
            .actions_of(kind)
            .filter(|a| action_ids.contains(a.id.as_str()))
            .collect()
    };

    match mode {
        Mode::Apply => {
            // Tweaks first: they may enable repos that apps need.
            push_actions(&mut plan, &actions_of(ActionKind::Tweak), mode);
            push_apps(&mut plan, catalog, family, aur, &app_ids, mode);
            push_actions(&mut plan, &actions_of(ActionKind::Setup), mode);
            push_actions(&mut plan, &actions_of(ActionKind::Maintenance), mode);
        }
        Mode::Revert => {
            push_apps(&mut plan, catalog, family, aur, &app_ids, mode);
            push_actions(&mut plan, &actions_of(ActionKind::Setup), mode);
            push_actions(&mut plan, &actions_of(ActionKind::Tweak), mode);
            push_actions(&mut plan, &actions_of(ActionKind::Maintenance), mode);
        }
    }
    Ok(plan)
}

fn push_apps(
    plan: &mut Plan,
    catalog: &Catalog,
    family: Family,
    aur: Option<&AurHelper>,
    ids: &HashSet<&str>,
    mode: Mode,
) {
    let mut native = (Vec::new(), Vec::new());
    let mut aur_pkgs = (Vec::new(), Vec::new());
    let mut flatpak = (Vec::new(), Vec::new());
    for app in catalog.apps.iter().filter(|a| ids.contains(a.id.as_str())) {
        let bucket = match app.install_method(family, aur.is_some()) {
            Some(InstallMethod::Native(p)) => (&mut native, p),
            Some(InstallMethod::Aur(p)) => (&mut aur_pkgs, p),
            Some(InstallMethod::Flatpak(p)) => (&mut flatpak, p),
            None => {
                plan.warnings.push(format!(
                    "{} has no install method on {family}; skipped",
                    app.name
                ));
                continue;
            }
        };
        bucket.0.0.push(app.name.clone());
        bucket.0.1.extend(bucket.1);
    }

    let verb = match mode {
        Mode::Apply => "Install",
        Mode::Revert => "Uninstall",
    };
    let mut add = |pm: &dyn PackageManager,
                   (names, pkgs): (Vec<String>, Vec<String>),
                   extra: Option<String>| {
        if pkgs.is_empty() {
            return;
        }
        let cmd = match mode {
            Mode::Apply => pm.install(&pkgs),
            Mode::Revert => pm.remove(&pkgs),
        };
        let command = match extra {
            Some(pre) => format!("{pre}\n{cmd}"),
            None => cmd,
        };
        plan.steps.push(Step {
            label: format!("{verb} {} ({})", names.join(", "), pm.name()),
            root: pm.needs_root(),
            command,
        });
    };

    add(pkg::native(family), native, None);
    if let Some(helper) = aur {
        add(helper, aur_pkgs, None);
    }
    let ensure = (mode == Mode::Apply).then(|| pkg::ensure_flatpak(family));
    add(&Flatpak, flatpak, ensure);
}

fn push_actions(plan: &mut Plan, actions: &[&Action], mode: Mode) {
    for a in actions {
        let inv = match mode {
            Mode::Apply => Some(a.run.as_str()),
            Mode::Revert => a.revert.as_deref(),
        };
        let Some(inv) = inv else {
            plan.warnings
                .push(format!("{} cannot be reverted; skipped", a.name));
            continue;
        };
        if let Some(c) = &a.confirm {
            plan.confirmations.push(format!("{}: {c}", a.name));
        }
        let prefix = if mode == Mode::Revert { "Revert: " } else { "" };
        plan.steps.push(Step {
            label: format!("{prefix}{}", a.name),
            root: a.root,
            command: script_command(inv),
        });
    }
}

/// `tweaks/foo.sh apply` → `bash "$NX_SCRIPTS"/tweaks/foo.sh apply`
pub fn script_command(invocation: &str) -> String {
    let (script, args) = split_invocation(invocation);
    let mut cmd = format!("bash \"$NX_SCRIPTS\"/{}", shell::quote(script));
    if !args.is_empty() {
        cmd.push(' ');
        cmd.push_str(&shell::join(&args));
    }
    cmd
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> Catalog {
        let apps: toml::Table = toml::from_str(
            r#"
            [[app]]
            id = "git"
            name = "Git"
            category = "Dev"
            packages = { fedora = "git", debian = "git", arch = "git" }

            [[app]]
            id = "discord"
            name = "Discord"
            category = "Chat"
            packages = { aur = "discord-bin", flatpak = "com.discordapp.Discord" }
        "#,
        )
        .unwrap();
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("apps.toml"),
            toml::to_string(&apps).unwrap(),
        )
        .unwrap();
        std::fs::write(
            dir.path().join("tweaks.toml"),
            r#"
            [[action]]
            id = "rpmfusion"
            name = "Enable RPM Fusion"
            distros = ["fedora"]
            root = true
            run = "tweaks/rpmfusion.sh apply"
            revert = "tweaks/rpmfusion.sh revert"
            confirm = "Adds third-party repos"
        "#,
        )
        .unwrap();
        std::fs::write(
            dir.path().join("maintenance.toml"),
            r#"
            [[action]]
            id = "update"
            name = "Update system"
            root = true
            run = "maintenance/update.sh"
        "#,
        )
        .unwrap();
        Catalog::load(dir.path()).unwrap()
    }

    fn ids(v: &[&str]) -> Vec<String> {
        v.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn fedora_apply_orders_tweaks_first() {
        let c = catalog().for_family(Family::Fedora, false);
        let p = build(
            &c,
            Family::Fedora,
            None,
            &ids(&["update", "git", "discord", "rpmfusion"]),
            Mode::Apply,
        )
        .unwrap();
        let labels: Vec<_> = p.steps.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(
            labels,
            [
                "Enable RPM Fusion",
                "Install Git (dnf)",
                "Install Discord (flatpak)",
                "Update system"
            ]
        );
        assert_eq!(
            p.steps[0].command,
            "bash \"$NX_SCRIPTS\"/tweaks/rpmfusion.sh apply"
        );
        assert!(p.steps[2].command.contains("command -v flatpak"));
        assert_eq!(p.confirmations.len(), 1);
        assert!(p.needs_root());
    }

    #[test]
    fn arch_uses_aur_as_user() {
        let c = catalog().for_family(Family::Arch, true);
        let helper = AurHelper { bin: "paru" };
        let p = build(
            &c,
            Family::Arch,
            Some(&helper),
            &ids(&["discord"]),
            Mode::Apply,
        )
        .unwrap();
        assert_eq!(p.steps.len(), 1);
        assert!(!p.steps[0].root);
        assert_eq!(
            p.steps[0].command,
            "paru -S --needed --noconfirm discord-bin"
        );
    }

    #[test]
    fn revert_skips_irreversible() {
        let c = catalog().for_family(Family::Fedora, false);
        let p = build(
            &c,
            Family::Fedora,
            None,
            &ids(&["update", "rpmfusion", "git"]),
            Mode::Revert,
        )
        .unwrap();
        let labels: Vec<_> = p.steps.iter().map(|s| s.label.as_str()).collect();
        assert_eq!(labels, ["Uninstall Git (dnf)", "Revert: Enable RPM Fusion"]);
        assert_eq!(p.warnings.len(), 1);
    }

    #[test]
    fn unknown_id() {
        let c = catalog();
        assert!(matches!(
            build(&c, Family::Fedora, None, &ids(&["nope"]), Mode::Apply),
            Err(Error::UnknownId(_))
        ));
    }

    #[test]
    fn filtered_catalog_rejects_other_distro_items() {
        let c = catalog().for_family(Family::Debian, false);
        assert!(build(&c, Family::Debian, None, &ids(&["rpmfusion"]), Mode::Apply).is_err());
    }
}
