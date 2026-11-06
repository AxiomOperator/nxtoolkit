#!/usr/bin/env bash
# shellcheck source=SCRIPTDIR/../common.sh
source "$(dirname "${BASH_SOURCE[0]}")/../common.sh"

CONF=/etc/systemd/zram-generator.conf

case "${1:-apply}" in
    apply)
        case "$NX_FAMILY" in
            debian) nx_pkg_install systemd-zram-generator ;;
            *)      nx_pkg_install zram-generator ;;
        esac
        nx_backup "$CONF"
        cat > "$CONF" <<'CONF'
# Written by nxtoolkit
[zram0]
zram-size = min(ram / 2, 8192)
compression-algorithm = zstd
CONF
        systemctl daemon-reload
        systemctl restart systemd-zram-setup@zram0.service
        swapon --show
        ;;
    revert)
        systemctl stop systemd-zram-setup@zram0.service || true
        if [ -f "$CONF.nxtoolkit.bak" ]; then mv "$CONF.nxtoolkit.bak" "$CONF"; else rm -f "$CONF"; fi
        systemctl daemon-reload
        ;;
    status)
        grep -qs zram /proc/swaps
        ;;
    *) nx_die "usage: $0 apply|revert|status" ;;
esac
