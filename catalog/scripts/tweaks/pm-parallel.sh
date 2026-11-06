#!/usr/bin/env bash
# Parallel downloads for dnf (fedora) or pacman (arch).
# shellcheck source=SCRIPTDIR/../common.sh
source "$(dirname "${BASH_SOURCE[0]}")/../common.sh"

case "$NX_FAMILY" in
    fedora)
        CONF=/etc/dnf/dnf.conf
        case "${1:-apply}" in
            apply)
                nx_backup "$CONF"
                [ -f "$CONF" ] || printf '[main]\n' > "$CONF"
                if grep -q '^max_parallel_downloads' "$CONF"; then
                    sed -i 's/^max_parallel_downloads.*/max_parallel_downloads=10/' "$CONF"
                else
                    sed -i '/^\[main\]/a max_parallel_downloads=10' "$CONF"
                fi
                ;;
            revert) sed -i '/^max_parallel_downloads/d' "$CONF" ;;
            status) grep -q '^max_parallel_downloads=10' "$CONF" ;;
            *) nx_die "usage: $0 apply|revert|status" ;;
        esac
        ;;
    arch)
        CONF=/etc/pacman.conf
        case "${1:-apply}" in
            apply)
                nx_backup "$CONF"
                sed -i -E 's/^#?ParallelDownloads.*/ParallelDownloads = 10/; s/^#Color$/Color/' "$CONF"
                grep -q '^ParallelDownloads' "$CONF" \
                    || sed -i '/^\[options\]/a ParallelDownloads = 10' "$CONF"
                ;;
            revert) sed -i -E 's/^ParallelDownloads.*/ParallelDownloads = 5/' "$CONF" ;;
            status) grep -q '^ParallelDownloads = 10' "$CONF" ;;
            *) nx_die "usage: $0 apply|revert|status" ;;
        esac
        ;;
    *) nx_die "unsupported: $NX_FAMILY" ;;
esac
