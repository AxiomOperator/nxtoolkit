#!/bin/sh
# nxtoolkit launcher: downloads the latest release binary and runs it.
#   curl -fsSL https://raw.githubusercontent.com/axiomoperator/nxtoolkit/main/install.sh | sh
set -eu

REPO="axiomoperator/nxtoolkit"
case "$(uname -m)" in
    x86_64)  arch=x86_64 ;;
    *) echo "nxtoolkit: unsupported architecture $(uname -m)" >&2; exit 1 ;;
esac

url="https://github.com/$REPO/releases/latest/download/nxtoolkit-$arch-linux-musl"
dir=$(mktemp -d)
trap 'rm -rf "$dir"' EXIT
echo "Downloading $url"
curl -fsSL "$url" -o "$dir/nxtoolkit"
chmod +x "$dir/nxtoolkit"
# Re-attach stdin to the terminal so the TUI works when piped from curl.
"$dir/nxtoolkit" "$@" </dev/tty
