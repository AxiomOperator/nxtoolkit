#!/usr/bin/env bash
# shellcheck source=SCRIPTDIR/../common.sh
source "$(dirname "${BASH_SOURCE[0]}")/../common.sh"

case "${1:-apply}" in
    apply)
        ver=$(rpm -E %fedora)
        dnf install -y \
            "https://mirrors.rpmfusion.org/free/fedora/rpmfusion-free-release-${ver}.noarch.rpm" \
            "https://mirrors.rpmfusion.org/nonfree/fedora/rpmfusion-nonfree-release-${ver}.noarch.rpm"
        ;;
    revert)
        dnf remove -y rpmfusion-free-release rpmfusion-nonfree-release
        ;;
    status)
        rpm -q --quiet rpmfusion-free-release && rpm -q --quiet rpmfusion-nonfree-release
        ;;
    *) nx_die "usage: $0 apply|revert|status" ;;
esac
