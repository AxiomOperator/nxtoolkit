#!/usr/bin/env bash
# shellcheck source=SCRIPTDIR/../common.sh
source "$(dirname "${BASH_SOURCE[0]}")/../common.sh"

case "${1:-apply}" in
    apply)
        nx_has flatpak || nx_pkg_install flatpak
        flatpak remote-add --if-not-exists flathub https://dl.flathub.org/repo/flathub.flatpakrepo
        nx_log "Flathub enabled"
        ;;
    revert)
        flatpak remote-delete --force flathub
        ;;
    status)
        nx_has flatpak && flatpak remotes --columns=name | grep -qx flathub
        ;;
    *) nx_die "usage: $0 apply|revert|status" ;;
esac
