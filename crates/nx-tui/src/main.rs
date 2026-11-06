//! `nxtoolkit`: terminal UI (default) and scriptable CLI.

mod cli;
mod tui;

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use nx_core::{Family, InitOptions, Toolkit};

#[derive(Parser)]
#[command(
    name = "nxtoolkit",
    version,
    about = "Install apps, tweak, and maintain your Linux system"
)]
struct Cli {
    /// Use catalog + scripts from DIR instead of the built-in copy.
    #[arg(long, global = true, value_name = "DIR")]
    catalog: Option<PathBuf>,
    /// Pretend to be this distro family (fedora, debian, arch). Implies dry runs.
    #[arg(long, global = true, value_name = "FAMILY")]
    distro: Option<Family>,
    #[command(subcommand)]
    command: Option<Command>,
}

#[derive(Subcommand)]
enum Command {
    /// Open the interactive terminal UI (default).
    Tui,
    /// List catalog entries available on this system.
    List {
        #[arg(long, value_enum)]
        kind: Option<ListKind>,
    },
    /// Install apps / apply actions by id.
    Apply {
        #[arg(required = true)]
        ids: Vec<String>,
        /// Uninstall apps / revert tweaks instead.
        #[arg(long)]
        revert: bool,
        /// Print the commands without running them.
        #[arg(long)]
        dry_run: bool,
        /// Don't ask for confirmation.
        #[arg(short, long)]
        yes: bool,
    },
    /// Check the catalog for missing scripts, duplicate ids, etc.
    Validate,
    /// Show the detected distro and environment.
    Info,
}

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum ListKind {
    Apps,
    Tweaks,
    Maintenance,
    Setup,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let opts = InitOptions {
        catalog_dir: cli.catalog,
        family_override: cli.distro,
    };
    let simulated = opts.family_override.is_some();
    let tk = Toolkit::init(&opts)?;
    match cli.command.unwrap_or(Command::Tui) {
        Command::Tui => tui::run(tk, simulated),
        Command::List { kind } => cli::list(&tk, kind),
        Command::Apply {
            ids,
            revert,
            dry_run,
            yes,
        } => {
            let code = cli::apply(&tk, &ids, revert, dry_run || simulated, yes)?;
            std::process::exit(code);
        }
        Command::Validate => {
            let code = cli::validate(&opts)?;
            std::process::exit(code);
        }
        Command::Info => cli::info(&tk),
    }
}
