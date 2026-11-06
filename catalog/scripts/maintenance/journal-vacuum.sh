#!/usr/bin/env bash
# shellcheck source=SCRIPTDIR/../common.sh
source "$(dirname "${BASH_SOURCE[0]}")/../common.sh"

journalctl --disk-usage
journalctl --vacuum-time=2weeks
journalctl --disk-usage
