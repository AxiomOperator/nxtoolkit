#!/usr/bin/env bash
# shellcheck source=SCRIPTDIR/../common.sh
source "$(dirname "${BASH_SOURCE[0]}")/../common.sh"

case "$NX_FAMILY" in
    fedora) dnf upgrade --refresh -y ;;
    debian) apt-get update -q && DEBIAN_FRONTEND=noninteractive apt-get full-upgrade -y ;;
    arch)   pacman -Syu --noconfirm ;;
    *)      nx_die "unsupported: $NX_FAMILY" ;;
esac

if nx_has flatpak; then
    nx_log "Updating Flatpaks"
    flatpak update -y --noninteractive
fi

if [ -e /var/run/reboot-required ] || { nx_has needs-restarting && ! needs-restarting -r >/dev/null 2>&1; }; then
    nx_warn "a reboot is required to finish applying updates"
fi
