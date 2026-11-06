//! Per-run log files under `$XDG_STATE_HOME/nxtoolkit/logs`.

use std::{
    fs::{self, File},
    io::{self, Write},
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

pub struct Logger {
    file: File,
    pub path: PathBuf,
}

pub fn log_dir() -> Option<PathBuf> {
    let base = std::env::var_os("XDG_STATE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".local/state")))?;
    Some(base.join("nxtoolkit/logs"))
}

impl Logger {
    pub fn create() -> io::Result<Self> {
        let dir = log_dir().ok_or_else(|| io::Error::other("no HOME"))?;
        fs::create_dir_all(&dir)?;
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        let path = dir.join(format!("run-{ts}-{}.log", std::process::id()));
        let file = File::create(&path)?;
        Ok(Logger { file, path })
    }

    pub fn line(&mut self, s: &str) {
        let _ = writeln!(self.file, "{s}");
    }
}
