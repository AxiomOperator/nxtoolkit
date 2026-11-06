//! nx-core: everything nxtoolkit does that is not UI.
//!
//! Frontends call [`Toolkit::init`], show the [`Catalog`] to the user, turn the
//! selection into a [`Plan`] with [`Toolkit::plan`], and execute it with
//! [`Toolkit::spawn_run`], rendering the [`runner::Event`] stream.

pub mod catalog;
pub mod check;
pub mod data;
pub mod distro;
pub mod log;
pub mod pkg;
pub mod plan;
pub mod privilege;
pub mod runner;
pub mod shell;

use std::path::PathBuf;

pub use catalog::{Action, ActionKind, App, Catalog};
pub use data::Data;
pub use distro::{Distro, Family};
pub use plan::{Mode, Plan, Step};
pub use privilege::Elevation;
pub use runner::Event;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("unsupported distribution: {0}")]
    UnsupportedDistro(String),
    #[error("unknown item id: {0}")]
    UnknownId(String),
    #[error("catalog error in {file}: {message}")]
    Catalog { file: String, message: String },
    #[error(transparent)]
    Io(#[from] std::io::Error),
}

/// Options shared by all frontends.
#[derive(Debug, Clone, Default)]
pub struct InitOptions {
    /// Use catalog + scripts from this directory instead of the embedded copy.
    pub catalog_dir: Option<PathBuf>,
    /// Pretend to be this distro family (for dry runs / testing).
    pub family_override: Option<Family>,
}

/// Loaded, distro-filtered toolkit state.
pub struct Toolkit {
    pub distro: Distro,
    pub data: Data,
    /// Catalog filtered to entries usable on `distro`.
    pub catalog: Catalog,
    pub aur: Option<pkg::AurHelper>,
}

impl Toolkit {
    pub fn init(opts: &InitOptions) -> anyhow::Result<Self> {
        let distro = match opts.family_override {
            Some(f) => Distro::synthetic(f),
            None => Distro::detect()?,
        };
        let data = Data::load(opts.catalog_dir.as_deref())?;
        let full = Catalog::load(&data.catalog_dir())?;
        let aur = if distro.family == Family::Arch {
            pkg::AurHelper::detect()
        } else {
            None
        };
        let catalog = full.for_family(distro.family, aur.is_some());
        Ok(Toolkit {
            distro,
            data,
            catalog,
            aur,
        })
    }

    /// Build an execution plan for the given item ids.
    pub fn plan(&self, ids: &[String], mode: Mode) -> Result<Plan, Error> {
        plan::build(
            &self.catalog,
            self.distro.family,
            self.aur.as_ref(),
            ids,
            mode,
        )
    }

    /// Run a plan on a background thread; events arrive on the returned channel.
    pub fn spawn_run(
        &self,
        plan: Plan,
        elevation: Elevation,
    ) -> crossbeam_channel::Receiver<Event> {
        let ctx = runner::RunContext {
            scripts_dir: self.data.scripts_dir(),
            distro: self.distro.clone(),
            elevation,
        };
        runner::spawn(plan, ctx)
    }

    /// Check (on a background thread) which actions with a `check` are already applied.
    pub fn spawn_checks(&self) -> crossbeam_channel::Receiver<(String, bool)> {
        check::spawn(
            self.catalog.actions.clone(),
            self.data.scripts_dir(),
            self.distro.clone(),
        )
    }
}
