#!/usr/bin/env bash
# Runs as the normal user.
# shellcheck source=SCRIPTDIR/../common.sh
source "$(dirname "${BASH_SOURCE[0]}")/../common.sh"

RUSTUP="${CARGO_HOME:-$HOME/.cargo}/bin/rustup"

case "${1:-apply}" in
    apply)
        if [ -x "$RUSTUP" ]; then
            "$RUSTUP" update
        else
            nx_has curl || nx_die "curl is required"
            curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        fi
        ;;
    revert)
        [ -x "$RUSTUP" ] || nx_die "rustup is not installed"
        "$RUSTUP" self uninstall -y
        ;;
    status)
        [ -x "$RUSTUP" ]
        ;;
    *) nx_die "usage: $0 apply|revert|status" ;;
esac
