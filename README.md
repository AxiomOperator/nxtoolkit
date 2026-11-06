# nxtoolkit

A Linux system utility in the spirit of [winutil](https://github.com/christitustech/winutil): pick apps to install, apply system tweaks, run maintenance and setup bundles, from a terminal UI or a desktop GUI.

Supports **Fedora/RHEL**, **Debian/Ubuntu**, and **Arch** (including derivatives such as Mint, Pop!_OS, EndeavourOS, and CachyOS).

| Tab | What it does |
|---|---|
| Install | ~40 apps. Uses the native package manager, then the AUR (if paru/yay is installed), then Flathub. |
| Tweaks | Repositories (Flathub, RPM Fusion, non-free, multilib), faster downloads, zram, TRIM, firewall. Each shows whether it's already applied and can be reverted. |
| Maintenance | Update the system, AUR packages and firmware; clean caches, orphans, Flatpak and the journal |
| Setup | NVIDIA driver, gaming bundle, build tools, rustup, nvm, Docker, Podman |

## Usage

```sh
nxtoolkit                         # terminal UI
nxtoolkit-gui                     # desktop GUI (polkit prompt for root steps)
nxtoolkit list [--kind tweaks]    # what's available on this distro
nxtoolkit apply firefox flathub update            # confirm, then run
nxtoolkit apply --revert rpmfusion                # undo a tweak / uninstall an app
nxtoolkit apply --dry-run steam nvidia            # print commands only
nxtoolkit --distro arch apply steam               # preview another distro (always a dry run)
nxtoolkit validate                                # check the catalog
```

TUI keys: `Tab`/`←→` switch tabs, `Space` select, `a` select all, `/` filter, `Enter` apply, `r` revert/uninstall, `?` help.

Every run logs to `~/.local/state/nxtoolkit/logs/`.

## How it works

```
crates/nx-core   distro detection, catalog, planning, privileged execution (no UI)
crates/nx-tui    `nxtoolkit` binary: ratatui TUI + CLI
crates/nx-gui    `nxtoolkit-gui` binary: egui
catalog/         *.toml + scripts/, embedded into the binaries at build time
```

1. The selected items become a **plan**: tweaks run first, then app installs batched per package manager, then setup, then maintenance. The plan is shown before anything runs.
2. Consecutive root steps are written into **one** bash script and run once through `sudo` (TUI/CLI) or `pkexec` (GUI), so you're asked for your password once. User-level steps, such as AUR helpers, rustup and nvm, run as you.
3. Output streams live to the UI and the log, and each step reports success or failure on its own.

## Adding to the catalog

Apps (`catalog/apps.toml`):

```toml
[[app]]
id = "firefox"
name = "Firefox"
category = "Browsers"
description = "Mozilla's open-source web browser."
packages = { fedora = "firefox", arch = "firefox", flatpak = "org.mozilla.firefox" }
```

Actions (`tweaks.toml`, `maintenance.toml`, `setup.toml`):

```toml
[[action]]
id = "fstrim"
name = "Weekly SSD TRIM"
category = "Performance"
root = true
distros = ["fedora", "debian", "arch"]   # omit for all
run    = "tweaks/fstrim.sh apply"
revert = "tweaks/fstrim.sh revert"      # optional
check  = "tweaks/fstrim.sh status"      # optional, unprivileged; exit 0 = applied
confirm = "Shown as a warning before running"  # optional
```

Scripts source `catalog/scripts/common.sh`, which provides `NX_FAMILY`, `nx_pkg_install`, `nx_backup`, `nx_log` and similar helpers. To test changes without rebuilding, run `cargo run -p nx-tui -- --catalog catalog`.

## Development

```sh
cargo test --workspace
cargo run -p nx-tui                       # TUI
cargo run -p nx-gui                       # GUI
cargo run -p nx-tui -- validate
shellcheck -x catalog/scripts/*.sh catalog/scripts/*/*.sh
```

CI (`.github/workflows/ci.yml`) runs fmt, clippy, tests, and shellcheck, then smoke tests inside Fedora, Ubuntu, Debian, and Arch containers. Tagging `v*` publishes a static musl binary for `install.sh`.
