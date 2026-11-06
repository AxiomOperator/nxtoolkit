#!/usr/bin/env bash
# Runs as the normal user: AUR helpers call sudo themselves.
# shellcheck source=SCRIPTDIR/../common.sh
source "$(dirname "${BASH_SOURCE[0]}")/../common.sh"

for h in paru yay; do
    if nx_has "$h"; then
        "$h" -Sua --noconfirm
        exit 0
    fi
done
nx_die "no AUR helper (paru or yay) found"
