//! Package manager command generation.
//!
//! These produce shell snippets rather than running anything, so the same
//! commands can be shown in a dry run and batched into one elevated script.

use crate::{Family, shell};

pub trait PackageManager {
    fn name(&self) -> &'static str;
    /// Whether this manager's commands must run as root.
    fn needs_root(&self) -> bool;
    fn install(&self, pkgs: &[String]) -> String;
    fn remove(&self, pkgs: &[String]) -> String;
    /// argv that exits 0 iff `pkg` is installed.
    fn is_installed_argv(&self, pkg: &str) -> Vec<String>;
}

pub struct Dnf;
pub struct Apt;
pub struct Pacman;
pub struct Flatpak;

#[derive(Debug, Clone)]
pub struct AurHelper {
    pub bin: &'static str,
}

impl PackageManager for Dnf {
    fn name(&self) -> &'static str {
        "dnf"
    }
    fn needs_root(&self) -> bool {
        true
    }
    fn install(&self, pkgs: &[String]) -> String {
        format!("dnf install -y {}", shell::join(pkgs))
    }
    fn remove(&self, pkgs: &[String]) -> String {
        format!("dnf remove -y {}", shell::join(pkgs))
    }
    fn is_installed_argv(&self, pkg: &str) -> Vec<String> {
        argv(&["rpm", "-q", "--quiet", pkg])
    }
}

impl PackageManager for Apt {
    fn name(&self) -> &'static str {
        "apt"
    }
    fn needs_root(&self) -> bool {
        true
    }
    fn install(&self, pkgs: &[String]) -> String {
        format!(
            "apt-get update && DEBIAN_FRONTEND=noninteractive apt-get install -y {}",
            shell::join(pkgs)
        )
    }
    fn remove(&self, pkgs: &[String]) -> String {
        format!(
            "DEBIAN_FRONTEND=noninteractive apt-get remove -y {}",
            shell::join(pkgs)
        )
    }
    fn is_installed_argv(&self, pkg: &str) -> Vec<String> {
        argv(&["dpkg", "-s", pkg])
    }
}

impl PackageManager for Pacman {
    fn name(&self) -> &'static str {
        "pacman"
    }
    fn needs_root(&self) -> bool {
        true
    }
    fn install(&self, pkgs: &[String]) -> String {
        format!("pacman -S --needed --noconfirm {}", shell::join(pkgs))
    }
    fn remove(&self, pkgs: &[String]) -> String {
        format!("pacman -Rns --noconfirm {}", shell::join(pkgs))
    }
    fn is_installed_argv(&self, pkg: &str) -> Vec<String> {
        argv(&["pacman", "-Qq", pkg])
    }
}

impl PackageManager for AurHelper {
    fn name(&self) -> &'static str {
        self.bin
    }
    /// AUR helpers refuse to run as root; they call sudo themselves.
    fn needs_root(&self) -> bool {
        false
    }
    fn install(&self, pkgs: &[String]) -> String {
        format!("{} -S --needed --noconfirm {}", self.bin, shell::join(pkgs))
    }
    fn remove(&self, pkgs: &[String]) -> String {
        format!("{} -Rns --noconfirm {}", self.bin, shell::join(pkgs))
    }
    fn is_installed_argv(&self, pkg: &str) -> Vec<String> {
        argv(&["pacman", "-Qq", pkg])
    }
}

impl AurHelper {
    pub fn detect() -> Option<Self> {
        ["paru", "yay"]
            .into_iter()
            .find(|b| which::which(b).is_ok())
            .map(|bin| AurHelper { bin })
    }
}

impl PackageManager for Flatpak {
    fn name(&self) -> &'static str {
        "flatpak"
    }
    fn needs_root(&self) -> bool {
        true
    }
    fn install(&self, pkgs: &[String]) -> String {
        format!(
            "flatpak remote-add --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo && \
             flatpak install -y --noninteractive flathub {}",
            shell::join(pkgs)
        )
    }
    fn remove(&self, pkgs: &[String]) -> String {
        format!(
            "flatpak uninstall -y --noninteractive {}",
            shell::join(pkgs)
        )
    }
    fn is_installed_argv(&self, pkg: &str) -> Vec<String> {
        argv(&["flatpak", "info", pkg])
    }
}

/// The distro's native package manager.
pub fn native(family: Family) -> &'static dyn PackageManager {
    match family {
        Family::Fedora => &Dnf,
        Family::Debian => &Apt,
        Family::Arch => &Pacman,
    }
}

/// Snippet that installs flatpak itself if it's missing.
pub fn ensure_flatpak(family: Family) -> String {
    format!(
        "command -v flatpak >/dev/null || {{ {}; }}",
        native(family).install(&["flatpak".to_string()])
    )
}

fn argv(a: &[&str]) -> Vec<String> {
    a.iter().map(|s| s.to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_commands() {
        let p = vec!["git".to_string(), "neovim".to_string()];
        assert_eq!(
            native(Family::Fedora).install(&p),
            "dnf install -y git neovim"
        );
        assert_eq!(
            native(Family::Arch).install(&p),
            "pacman -S --needed --noconfirm git neovim"
        );
        assert!(
            native(Family::Debian)
                .install(&p)
                .ends_with("apt-get install -y git neovim")
        );
        assert!(
            Flatpak
                .install(&["com.discordapp.Discord".into()])
                .contains("flathub com.discordapp.Discord")
        );
        assert!(!AurHelper { bin: "paru" }.needs_root());
    }
}
