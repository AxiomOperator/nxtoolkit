#!/usr/bin/env bash
# shellcheck source=SCRIPTDIR/../common.sh
source "$(dirname "${BASH_SOURCE[0]}")/../common.sh"

has_nvidia_gpu() {
    grep -qs '0x10de' /sys/bus/pci/devices/*/vendor
}

case "${1:-apply}" in
    status)
        nx_has nvidia-smi
        exit $?
        ;;
    apply) ;;
    *) nx_die "usage: $0 apply|status" ;;
esac

has_nvidia_gpu || nx_die "no NVIDIA GPU detected"

case "$NX_FAMILY" in
    fedora)
        rpm -q --quiet rpmfusion-nonfree-release \
            || bash "$(dirname "${BASH_SOURCE[0]}")/../tweaks/rpmfusion.sh" apply
        dnf install -y akmod-nvidia xorg-x11-drv-nvidia-cuda
        nx_log "Building the kernel module (akmods)..."
        akmods --force || true
        ;;
    debian)
        if nx_is_ubuntu; then
            nx_pkg_install ubuntu-drivers-common
            ubuntu-drivers install
        else
            bash "$(dirname "${BASH_SOURCE[0]}")/../tweaks/debian-nonfree.sh" apply
            nx_pkg_install "linux-headers-$(uname -r)" nvidia-driver firmware-misc-nonfree
        fi
        ;;
    arch)
        pkgs=(nvidia-open-dkms nvidia-utils nvidia-settings)
        for k in linux linux-lts linux-zen linux-hardened; do
            pacman -Qq "$k" >/dev/null 2>&1 && pkgs+=("$k-headers")
        done
        grep -q '^\[multilib\]' /etc/pacman.conf && pkgs+=(lib32-nvidia-utils)
        pacman -S --needed --noconfirm "${pkgs[@]}"
        ;;
    *) nx_die "unsupported: $NX_FAMILY" ;;
esac
nx_warn "reboot to load the NVIDIA driver"
