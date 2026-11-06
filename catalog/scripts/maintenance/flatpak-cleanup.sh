#!/usr/bin/env bash
# shellcheck source=SCRIPTDIR/../common.sh
source "$(dirname "${BASH_SOURCE[0]}")/../common.sh"

if ! nx_has flatpak; then
    nx_log "flatpak is not installed; nothing to do"
    exit 0
fi
flatpak uninstall --unused -y --noninteractive
flatpak repair
