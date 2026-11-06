//! Where the catalog and scripts live: embedded in the binary, or a directory on disk.

use std::path::{Path, PathBuf};

use include_dir::{Dir, include_dir};
use tempfile::TempDir;

static EMBEDDED: Dir<'static> = include_dir!("$CARGO_MANIFEST_DIR/../../catalog");

pub struct Data {
    root: PathBuf,
    // Keeps the extracted embedded copy alive for the program's lifetime.
    _tmp: Option<TempDir>,
}

impl Data {
    /// Use `dir` if given, otherwise extract the embedded catalog to a temp dir.
    pub fn load(dir: Option<&Path>) -> std::io::Result<Self> {
        match dir {
            Some(d) => {
                let root = d.canonicalize()?;
                Ok(Data { root, _tmp: None })
            }
            None => {
                let tmp = tempfile::Builder::new().prefix("nxtoolkit-").tempdir()?;
                EMBEDDED.extract(tmp.path())?;
                Ok(Data {
                    root: tmp.path().to_path_buf(),
                    _tmp: Some(tmp),
                })
            }
        }
    }

    pub fn catalog_dir(&self) -> PathBuf {
        self.root.clone()
    }

    pub fn scripts_dir(&self) -> PathBuf {
        self.root.join("scripts")
    }
}
