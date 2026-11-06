#!/usr/bin/env bash
# shellcheck source=SCRIPTDIR/../common.sh
source "$(dirname "${BASH_SOURCE[0]}")/../common.sh"

case "${1:-apply}" in
    apply)  systemctl enable --now fstrim.timer ;;
    revert) systemctl disable --now fstrim.timer ;;
    status) systemctl is-enabled --quiet fstrim.timer ;;
    *) nx_die "usage: $0 apply|revert|status" ;;
esac
