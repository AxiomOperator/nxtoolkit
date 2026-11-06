#!/usr/bin/env bash
# shellcheck source=SCRIPTDIR/../common.sh
source "$(dirname "${BASH_SOURCE[0]}")/../common.sh"

nx_has fwupdmgr || nx_pkg_install fwupd
fwupdmgr refresh --force || true
fwupdmgr get-updates || { nx_log "no firmware updates available"; exit 0; }
fwupdmgr update --assume-yes --no-reboot-check
