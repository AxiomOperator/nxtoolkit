#!/usr/bin/env bash
# shellcheck source=SCRIPTDIR/../common.sh
source "$(dirname "${BASH_SOURCE[0]}")/../common.sh"

case "$NX_FAMILY" in
    fedora)
        case "${1:-apply}" in
            apply)
                nx_pkg_install firewalld
                systemctl enable --now firewalld
                firewall-cmd --state
                ;;
            revert) systemctl disable --now firewalld ;;
            status) systemctl is-active --quiet firewalld ;;
            *) nx_die "usage: $0 apply|revert|status" ;;
        esac
        ;;
    *)
        case "${1:-apply}" in
            apply)
                nx_pkg_install ufw
                ufw default deny incoming
                ufw default allow outgoing
                ufw --force enable
                systemctl enable ufw
                ufw status verbose
                ;;
            revert)
                ufw --force disable
                systemctl disable ufw
                ;;
            status)
                # `ufw status` needs root; approximate with the service state.
                systemctl is-enabled --quiet ufw && systemctl is-active --quiet ufw
                ;;
            *) nx_die "usage: $0 apply|revert|status" ;;
        esac
        ;;
esac
