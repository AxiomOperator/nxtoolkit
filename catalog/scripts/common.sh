#!/usr/bin/env bash
# Shared helpers for nxtoolkit scripts. Source with:
#   # shellcheck source=../common.sh
#   source "$(dirname "${BASH_SOURCE[0]}")/../common.sh"
#
# Environment provided by nxtoolkit (detected here if missing):
#   NX_FAMILY   fedora | debian | arch
#   NX_DISTRO_ID, NX_DISTRO_VERSION, NX_USER, NX_HOME, NX_SCRIPTS

set -euo pipefail

nx_log()  { printf '==> %s\n' "$*"; }
nx_warn() { printf 'warning: %s\n' "$*" >&2; }
nx_die()  { printf 'error: %s\n' "$*" >&2; exit 1; }
nx_has()  { command -v "$1" >/dev/null 2>&1; }

nx_require_root() {
    [ "$(id -u)" -eq 0 ] || nx_die "this action must run as root"
}

_nx_detect() {
    local id="" like=""
    if [ -r /etc/os-release ]; then
        # shellcheck disable=SC1091
        id=$(. /etc/os-release; echo "${ID:-}")
        # shellcheck disable=SC1091
        like=$(. /etc/os-release; echo "${ID_LIKE:-}")
    fi
    for t in $id $like; do
        case "$t" in
            fedora|rhel|centos|rocky|almalinux|nobara|ultramarine) echo fedora; return ;;
            debian|ubuntu|linuxmint|pop|elementary|zorin|kali|raspbian) echo debian; return ;;
            arch|archlinux|endeavouros|manjaro|cachyos|garuda) echo arch; return ;;
        esac
    done
    echo unknown
}

if [ -z "${NX_FAMILY:-}" ]; then
    NX_FAMILY=$(_nx_detect)
fi
if [ -z "${NX_DISTRO_ID:-}" ]; then
    # shellcheck disable=SC1091
    NX_DISTRO_ID=$( [ -r /etc/os-release ] && . /etc/os-release; echo "${ID:-unknown}")
fi
NX_USER=${NX_USER:-${SUDO_USER:-${USER:-}}}
export NX_FAMILY NX_DISTRO_ID NX_USER

nx_is_ubuntu() {
    [ "$NX_DISTRO_ID" = ubuntu ] || grep -qs '^ID_LIKE=.*ubuntu' /etc/os-release
}

nx_pkg_install() {
    case "$NX_FAMILY" in
        fedora) dnf install -y "$@" ;;
        debian) apt-get update -q && DEBIAN_FRONTEND=noninteractive apt-get install -y "$@" ;;
        arch)   pacman -S --needed --noconfirm "$@" ;;
        *)      nx_die "unsupported distro family: $NX_FAMILY" ;;
    esac
}

nx_pkg_remove() {
    case "$NX_FAMILY" in
        fedora) dnf remove -y "$@" ;;
        debian) DEBIAN_FRONTEND=noninteractive apt-get remove -y "$@" ;;
        arch)   pacman -Rns --noconfirm "$@" ;;
        *)      nx_die "unsupported distro family: $NX_FAMILY" ;;
    esac
}

nx_pkg_installed() {
    case "$NX_FAMILY" in
        fedora) rpm -q --quiet "$1" ;;
        debian) dpkg-query -W -f='${Status}' "$1" 2>/dev/null | grep -q 'install ok installed' ;;
        arch)   pacman -Qq "$1" >/dev/null 2>&1 ;;
        *)      return 1 ;;
    esac
}

# Back up a file once (first backup wins) to FILE.nxtoolkit.bak.
nx_backup() {
    [ -e "$1" ] && [ ! -e "$1.nxtoolkit.bak" ] && cp -a "$1" "$1.nxtoolkit.bak"
    return 0
}

# Run a command as the invoking (non-root) user.
nx_as_user() {
    if [ "$(id -u)" -eq 0 ] && [ -n "${NX_USER:-}" ] && [ "$NX_USER" != root ]; then
        runuser -u "$NX_USER" -- "$@"
    else
        "$@"
    fi
}
