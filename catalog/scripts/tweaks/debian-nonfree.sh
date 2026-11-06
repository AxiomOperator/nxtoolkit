#!/usr/bin/env bash
# shellcheck source=SCRIPTDIR/../common.sh
source "$(dirname "${BASH_SOURCE[0]}")/../common.sh"

DEB822=/etc/apt/sources.list.d/debian.sources
UBUNTU822=/etc/apt/sources.list.d/ubuntu.sources

has_components() {
    # $1 = file, rest = components that must all appear on a Components:/deb line
    local f=$1; shift
    [ -f "$f" ] || return 1
    local c
    for c in "$@"; do
        grep -Eq "^(Components:|deb ).*[[:space:]]$c([[:space:]]|$)" "$f" || return 1
    done
}

case "${1:-apply}" in
    apply)
        if nx_is_ubuntu; then
            nx_has add-apt-repository || nx_pkg_install software-properties-common
            add-apt-repository -y universe
            add-apt-repository -y multiverse
        elif [ -f "$DEB822" ]; then
            nx_backup "$DEB822"
            sed -i -E '/^Components:/ { s/ (contrib|non-free-firmware|non-free)//g; s/$/ contrib non-free non-free-firmware/ }' "$DEB822"
        else
            nx_backup /etc/apt/sources.list
            sed -i -E '/^deb(-src)? / { s/ (contrib|non-free-firmware|non-free)//g; s/$/ contrib non-free non-free-firmware/ }' /etc/apt/sources.list
        fi
        apt-get update -q
        ;;
    status)
        if nx_is_ubuntu; then
            has_components "$UBUNTU822" universe multiverse \
                || grep -rqsE '^deb .* multiverse' /etc/apt/sources.list /etc/apt/sources.list.d/
        elif [ -f "$DEB822" ]; then
            has_components "$DEB822" contrib non-free
        else
            has_components /etc/apt/sources.list contrib non-free
        fi
        ;;
    *) nx_die "usage: $0 apply|status" ;;
esac
