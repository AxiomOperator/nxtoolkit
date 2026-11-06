#!/usr/bin/env bash
# shellcheck source=SCRIPTDIR/../common.sh
source "$(dirname "${BASH_SOURCE[0]}")/../common.sh"

CONF=/etc/sysctl.d/99-nxtoolkit-swappiness.conf

case "${1:-apply}" in
    apply)
        echo 'vm.swappiness = 10' > "$CONF"
        sysctl -p "$CONF"
        ;;
    revert)
        rm -f "$CONF"
        sysctl --system >/dev/null
        nx_log "vm.swappiness is now $(cat /proc/sys/vm/swappiness)"
        ;;
    status)
        [ -f "$CONF" ] && [ "$(cat /proc/sys/vm/swappiness)" = 10 ]
        ;;
    *) nx_die "usage: $0 apply|revert|status" ;;
esac
