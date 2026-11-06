//! Running things as root.

use std::{
    io,
    path::Path,
    process::{Command, Stdio},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Elevation {
    /// Already root; run directly.
    AlreadyRoot,
    /// `sudo -n` (credentials must be cached first with [`Elevation::prepare`]).
    Sudo,
    /// `pkexec` (graphical polkit prompt).
    Pkexec,
}

pub fn is_root() -> bool {
    // SAFETY: geteuid has no preconditions and cannot fail.
    unsafe { libc::geteuid() == 0 }
}

impl Elevation {
    /// Pick an elevation method for a terminal or graphical frontend.
    pub fn for_frontend(graphical: bool) -> Self {
        if is_root() {
            Elevation::AlreadyRoot
        } else if graphical {
            Elevation::Pkexec
        } else {
            Elevation::Sudo
        }
    }

    /// For sudo: prompt for the password on the controlling terminal now, so
    /// the actual run can be non-interactive. Must be called while the
    /// terminal is in normal (cooked) mode.
    pub fn prepare(self) -> io::Result<bool> {
        match self {
            Elevation::Sudo => Ok(Command::new("sudo")
                .args(["-v", "-p", "[nxtoolkit] sudo password for %u: "])
                .status()?
                .success()),
            _ => Ok(true),
        }
    }

    /// Build the command that runs `script` with bash as root.
    pub fn command(self, script: &Path) -> Command {
        let mut cmd = match self {
            Elevation::AlreadyRoot => Command::new("bash"),
            Elevation::Sudo => {
                let mut c = Command::new("sudo");
                c.args(["-n", "--", "bash"]);
                c
            }
            Elevation::Pkexec => {
                // pkexec requires an absolute program path.
                let bash = which::which("bash").unwrap_or_else(|_| "/bin/bash".into());
                let mut c = Command::new("pkexec");
                c.arg(bash);
                c
            }
        };
        cmd.arg(script).stdin(Stdio::null());
        cmd
    }
}
