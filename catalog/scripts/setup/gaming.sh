#!/usr/bin/env bash
# shellcheck source=SCRIPTDIR/../common.sh
source "$(dirname "${BASH_SOURCE[0]}")/../common.sh"

here=$(dirname "${BASH_SOURCE[0]}")

case "$NX_FAMILY" in
    fedora)
        rpm -q --quiet rpmfusion-nonfree-release || bash "$here/../tweaks/rpmfusion.sh" apply
        dnf install -y steam lutris gamemode mangohud
        ;;
    debian)
        nx_pkg_install lutris gamemode mangohud
        bash "$here/../tweaks/flathub.sh" apply
        flatpak install -y --noninteractive flathub com.valvesoftware.Steam
        ;;
    arch)
        grep -q '^\[multilib\]' /etc/pacman.conf || bash "$here/../tweaks/arch-multilib.sh" apply
        pacman -S --needed --noconfirm steam lutris gamemode lib32-gamemode mangohud lib32-mangohud
        ;;
    *) nx_die "unsupported: $NX_FAMILY" ;;
esac

if [ -n "${NX_USER:-}" ] && [ "$NX_USER" != root ] && getent group gamemode >/dev/null; then
    usermod -aG gamemode "$NX_USER"
fi
