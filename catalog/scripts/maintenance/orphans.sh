#!/usr/bin/env bash
# shellcheck source=SCRIPTDIR/../common.sh
source "$(dirname "${BASH_SOURCE[0]}")/../common.sh"

case "$NX_FAMILY" in
    fedora) dnf autoremove -y ;;
    debian) DEBIAN_FRONTEND=noninteractive apt-get autoremove --purge -y ;;
    arch)
        mapfile -t orphans < <(pacman -Qdtq || true)
        if [ "${#orphans[@]}" -eq 0 ]; then
            nx_log "no orphaned packages"
        else
            nx_log "removing: ${orphans[*]}"
            pacman -Rns --noconfirm "${orphans[@]}"
        fi
        ;;
    *) nx_die "unsupported: $NX_FAMILY" ;;
esac
