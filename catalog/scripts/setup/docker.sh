#!/usr/bin/env bash
# shellcheck source=SCRIPTDIR/../common.sh
source "$(dirname "${BASH_SOURCE[0]}")/../common.sh"

case "${1:-apply}" in
    apply)
        case "$NX_FAMILY" in
            fedora) dnf install -y moby-engine docker-compose ;;
            debian)
                nx_pkg_install docker.io
                DEBIAN_FRONTEND=noninteractive apt-get install -y docker-compose-v2 \
                    || DEBIAN_FRONTEND=noninteractive apt-get install -y docker-compose
                ;;
            arch) nx_pkg_install docker docker-compose ;;
            *) nx_die "unsupported: $NX_FAMILY" ;;
        esac
        systemctl enable --now docker
        if [ -n "${NX_USER:-}" ] && [ "$NX_USER" != root ]; then
            usermod -aG docker "$NX_USER"
            nx_log "added $NX_USER to the docker group (log out and back in)"
        fi
        docker --version
        ;;
    revert)
        systemctl disable --now docker docker.socket || true
        if [ -n "${NX_USER:-}" ] && [ "$NX_USER" != root ]; then
            gpasswd -d "$NX_USER" docker || true
        fi
        nx_log "Docker disabled; packages and images were left in place"
        ;;
    status)
        nx_has docker && systemctl is-enabled --quiet docker
        ;;
    *) nx_die "usage: $0 apply|revert|status" ;;
esac
