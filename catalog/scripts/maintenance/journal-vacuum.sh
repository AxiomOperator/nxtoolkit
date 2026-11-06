#!/usr/bin/env bash
# shellcheck source=SCRIPTDIR/../common.sh
source "$(dirname "${BASH_SOURCE[0]}")/../common.sh"

if ! nx_has journalctl; then
    nx_log "systemd-journald is not present; nothing to do"
    exit 0
fi
journalctl --disk-usage
journalctl --vacuum-time=2weeks
journalctl --disk-usage
