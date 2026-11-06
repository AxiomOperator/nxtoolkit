#!/usr/bin/env bash
# shellcheck source=SCRIPTDIR/../common.sh
source "$(dirname "${BASH_SOURCE[0]}")/../common.sh"

case "$NX_FAMILY" in
    fedora) dnf install -y gcc gcc-c++ make cmake pkgconf-pkg-config git curl openssl-devel ;;
    debian) nx_pkg_install build-essential cmake pkg-config git curl libssl-dev ;;
    arch)   nx_pkg_install base-devel cmake git curl openssl ;;
    *)      nx_die "unsupported: $NX_FAMILY" ;;
esac
