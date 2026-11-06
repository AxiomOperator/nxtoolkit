//! Validates the shipped catalog: ids, script references, and per-distro plans.

use std::path::PathBuf;

use nx_core::{Catalog, Family, Mode, plan};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../catalog")
}

fn load() -> Catalog {
    Catalog::load(&root()).expect("catalog parses")
}

#[test]
fn catalog_is_valid() {
    let c = load();
    let problems = c.validate(&root().join("scripts"));
    assert!(
        problems.is_empty(),
        "catalog problems:\n{}",
        problems.join("\n")
    );
    assert!(c.apps.len() >= 30, "expected a full app catalog");
}

#[test]
fn every_family_can_plan_everything_it_shows() {
    let full = load();
    for family in Family::ALL {
        for has_aur in [false, true] {
            let c = full.for_family(family, has_aur);
            assert!(!c.apps.is_empty(), "{family}: no apps");
            let ids: Vec<String> = c
                .apps
                .iter()
                .map(|a| a.id.clone())
                .chain(c.actions.iter().map(|a| a.id.clone()))
                .collect();
            let aur = has_aur.then_some(nx_core::pkg::AurHelper { bin: "paru" });
            let aur = if family == Family::Arch { aur } else { None };
            for mode in [Mode::Apply, Mode::Revert] {
                let p = plan::build(&c, family, aur.as_ref(), &ids, mode)
                    .unwrap_or_else(|e| panic!("{family} {mode:?}: {e}"));
                assert!(!p.is_empty());
            }
        }
    }
}

#[test]
fn family_specific_actions_are_filtered() {
    let full = load();
    let deb = full.for_family(Family::Debian, false);
    assert!(deb.action("rpmfusion").is_none());
    assert!(deb.action("arch-multilib").is_none());
    assert!(deb.action("debian-nonfree").is_some());
    let fed = full.for_family(Family::Fedora, false);
    assert!(fed.action("rpmfusion").is_some());
    assert!(fed.app("brave").is_some(), "falls back to flatpak");
}
