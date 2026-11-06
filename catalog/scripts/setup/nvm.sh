#!/usr/bin/env bash
# Runs as the normal user.
# shellcheck source=SCRIPTDIR/../common.sh
source "$(dirname "${BASH_SOURCE[0]}")/../common.sh"

NVM_VERSION=v0.40.1
NVM_DIR="${NVM_DIR:-$HOME/.nvm}"

case "${1:-apply}" in
    apply)
        if [ ! -s "$NVM_DIR/nvm.sh" ]; then
            # shellcheck disable=SC2218  # false positive: defined in common.sh
            nx_has curl || nx_die "curl is required"
            curl -fsSL "https://raw.githubusercontent.com/nvm-sh/nvm/${NVM_VERSION}/install.sh" | bash
        fi
        # nvm.sh is not written for `set -u`.
        set +u
        # shellcheck disable=SC1091
        . "$NVM_DIR/nvm.sh"
        nvm install --lts
        nvm alias default 'lts/*'
        node --version
        ;;
    status)
        [ -s "$NVM_DIR/nvm.sh" ]
        ;;
    *) nx_die "usage: $0 apply|status" ;;
esac
