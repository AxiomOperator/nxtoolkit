#!/usr/bin/env bash
# shellcheck source=SCRIPTDIR/../common.sh
source "$(dirname "${BASH_SOURCE[0]}")/../common.sh"

case "$NX_FAMILY" in
    fedora) dnf clean all ;;
    debian) apt-get autoclean -y && apt-get clean ;;
    arch)
        if nx_has paccache; then
            paccache -rk2
            paccache -ruk0
        else
            pacman -Sc --noconfirm
        fi
        ;;
    *) nx_die "unsupported: $NX_FAMILY" ;;
esac
