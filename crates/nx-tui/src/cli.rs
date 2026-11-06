//! Non-interactive subcommands.

use std::io::{self, IsTerminal, Write};

use nx_core::{
    ActionKind, Catalog, Elevation, Event, Family, InitOptions, Mode, Toolkit,
    catalog::InstallMethod, data::Data, privilege,
};

use crate::ListKind;

pub fn list(tk: &Toolkit, kind: Option<ListKind>) -> anyhow::Result<()> {
    let family = tk.distro.family;
    let show = |k: ListKind| kind.is_none_or(|x| x == k);
    if show(ListKind::Apps) {
        println!("# Apps");
        for a in &tk.catalog.apps {
            let how = match a.install_method(family, tk.aur.is_some()) {
                Some(InstallMethod::Native(_)) => "native",
                Some(InstallMethod::Aur(_)) => "aur",
                Some(InstallMethod::Flatpak(_)) => "flatpak",
                None => "-",
            };
            println!("{:<18} {:<28} {:<14} {how}", a.id, a.name, a.category);
        }
    }
    for (lk, ak) in [
        (ListKind::Tweaks, ActionKind::Tweak),
        (ListKind::Maintenance, ActionKind::Maintenance),
        (ListKind::Setup, ActionKind::Setup),
    ] {
        if show(lk) {
            println!("\n# {}", ak.title());
            for a in tk.catalog.actions_of(ak) {
                let flags = format!(
                    "{}{}",
                    if a.root { "root" } else { "user" },
                    if a.revert.is_some() {
                        ",revertible"
                    } else {
                        ""
                    }
                );
                println!("{:<18} {:<40} {flags}", a.id, a.name);
            }
        }
    }
    Ok(())
}

pub fn info(tk: &Toolkit) -> anyhow::Result<()> {
    let d = &tk.distro;
    println!("distro:   {} (id={}, family={})", d.name, d.id, d.family);
    if let Some(v) = &d.version_id {
        println!("version:  {v}");
    }
    println!("aur:      {}", tk.aur.as_ref().map_or("none", |a| a.bin));
    println!("root:     {}", privilege::is_root());
    println!(
        "catalog:  {} apps, {} actions",
        tk.catalog.apps.len(),
        tk.catalog.actions.len()
    );
    if let Some(dir) = nx_core::log::log_dir() {
        println!("logs:     {}", dir.display());
    }
    Ok(())
}

pub fn validate(opts: &InitOptions) -> anyhow::Result<i32> {
    let data = Data::load(opts.catalog_dir.as_deref())?;
    let catalog = Catalog::load(&data.catalog_dir())?;
    let problems = catalog.validate(&data.scripts_dir());
    for p in &problems {
        eprintln!("error: {p}");
    }
    let per_family: Vec<String> = Family::ALL
        .iter()
        .map(|f| {
            let c = catalog.for_family(*f, false);
            format!("{f}: {} apps / {} actions", c.apps.len(), c.actions.len())
        })
        .collect();
    println!(
        "{} apps, {} actions ({})",
        catalog.apps.len(),
        catalog.actions.len(),
        per_family.join(", ")
    );
    if problems.is_empty() {
        println!("catalog OK");
        Ok(0)
    } else {
        Ok(1)
    }
}

pub fn apply(
    tk: &Toolkit,
    ids: &[String],
    revert: bool,
    dry_run: bool,
    yes: bool,
) -> anyhow::Result<i32> {
    let mode = if revert { Mode::Revert } else { Mode::Apply };
    let plan = tk.plan(ids, mode)?;
    if plan.is_empty() {
        for w in &plan.warnings {
            eprintln!("warning: {w}");
        }
        println!("nothing to do");
        return Ok(0);
    }
    println!("Plan for {}:\n", tk.distro.name);
    print!("{}", plan.render());
    if dry_run {
        return Ok(0);
    }
    if !yes {
        if !io::stdin().is_terminal() {
            anyhow::bail!("refusing to run without a terminal; pass --yes");
        }
        print!("\nProceed? [y/N] ");
        io::stdout().flush()?;
        let mut answer = String::new();
        io::stdin().read_line(&mut answer)?;
        if !matches!(answer.trim(), "y" | "Y" | "yes") {
            println!("aborted");
            return Ok(1);
        }
    }

    let elevation = Elevation::for_frontend(false);
    if plan.needs_root() && !elevation.prepare()? {
        anyhow::bail!("sudo authentication failed");
    }
    let total = plan.steps.len();
    let labels: Vec<String> = plan.steps.iter().map(|s| s.label.clone()).collect();
    let rx = tk.spawn_run(plan, elevation);
    let color = io::stdout().is_terminal();
    let (bold, green, red, reset) = if color {
        ("\x1b[1m", "\x1b[32m", "\x1b[31m", "\x1b[0m")
    } else {
        ("", "", "", "")
    };
    for ev in rx {
        match ev {
            Event::StepStarted { index } => {
                println!(
                    "\n{bold}==> [{}/{total}] {}{reset}",
                    index + 1,
                    labels[index]
                );
            }
            Event::Output { line } => println!("{line}"),
            Event::StepFinished { index, success } => {
                if success {
                    println!("{green}✓ {}{reset}", labels[index]);
                } else {
                    println!("{red}✗ {} failed{reset}", labels[index]);
                }
            }
            Event::Finished { failed, log } => {
                println!();
                if failed.is_empty() {
                    println!("{green}{bold}All {total} steps succeeded.{reset}");
                } else {
                    println!(
                        "{red}{bold}{} of {total} steps failed.{reset}",
                        failed.len()
                    );
                }
                if let Some(log) = log {
                    println!("log: {}", log.display());
                }
                return Ok(if failed.is_empty() { 0 } else { 1 });
            }
        }
    }
    Ok(1)
}
