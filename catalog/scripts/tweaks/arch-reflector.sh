#!/usr/bin/env bash
# shellcheck source=SCRIPTDIR/../common.sh
source "$(dirname "${BASH_SOURCE[0]}")/../common.sh"

LIST=/etc/pacman.d/mirrorlist

case "${1:-apply}" in
    apply)
        nx_pkg_install reflector
        nx_backup "$LIST"
        nx_log "Ranking mirrors (this takes a minute)..."
        reflector --latest 20 --protocol https --sort rate --save "$LIST"
        pacman -Syy --noconfirm
        ;;
    revert)
        [ -f "$LIST.nxtoolkit.bak" ] || nx_die "no backup at $LIST.nxtoolkit.bak"
        cp -a "$LIST.nxtoolkit.bak" "$LIST"
        pacman -Syy --noconfirm
        ;;
    *) nx_die "usage: $0 apply|revert" ;;
esac
