#!/usr/bin/env bash
# shellcheck source=SCRIPTDIR/../common.sh
source "$(dirname "${BASH_SOURCE[0]}")/../common.sh"

CONF=/etc/pacman.conf

case "${1:-apply}" in
    apply)
        if grep -q '^\[multilib\]' "$CONF"; then
            nx_log "multilib already enabled"
        else
            nx_backup "$CONF"
            # Uncomment "#[multilib]" and the Include line right after it.
            sed -i '/^#\[multilib\]/{s/^#//;n;s/^#//}' "$CONF"
            grep -q '^\[multilib\]' "$CONF" \
                || printf '\n[multilib]\nInclude = /etc/pacman.d/mirrorlist\n' >> "$CONF"
        fi
        pacman -Sy --noconfirm
        ;;
    revert)
        sed -i '/^\[multilib\]/{s/^/#/;n;s/^/#/}' "$CONF"
        pacman -Sy --noconfirm
        ;;
    status)
        grep -q '^\[multilib\]' "$CONF"
        ;;
    *) nx_die "usage: $0 apply|revert|status" ;;
esac
