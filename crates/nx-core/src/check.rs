//! Run actions' `check` scripts to show which tweaks are already applied.

use std::{
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use crossbeam_channel::{Receiver, unbounded};

use crate::{
    Distro,
    catalog::{Action, split_invocation},
    runner::script_env,
};

const TIMEOUT: Duration = Duration::from_secs(5);

/// Run one action's check. `None` if it has no check or the check timed out.
pub fn is_applied(action: &Action, scripts_dir: &std::path::Path, distro: &Distro) -> Option<bool> {
    let (script, args) = split_invocation(action.check.as_deref()?);
    let mut child = Command::new("bash")
        .arg(scripts_dir.join(script))
        .args(args)
        .envs(script_env(scripts_dir, distro))
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .ok()?;
    let start = Instant::now();
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Some(status.success()),
            Ok(None) if start.elapsed() < TIMEOUT => thread::sleep(Duration::from_millis(20)),
            _ => {
                let _ = child.kill();
                let _ = child.wait();
                return None;
            }
        }
    }
}

pub fn spawn(
    actions: Vec<Action>,
    scripts_dir: PathBuf,
    distro: Distro,
) -> Receiver<(String, bool)> {
    let (tx, rx) = unbounded();
    thread::spawn(move || {
        for a in actions.iter().filter(|a| a.check.is_some()) {
            if let Some(ok) = is_applied(a, &scripts_dir, &distro)
                && tx.send((a.id.clone(), ok)).is_err()
            {
                break;
            }
        }
    });
    rx
}
