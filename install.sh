#!/bin/sh
# install.sh — install the latest `ask` release.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/lev-gc/ask/main/install.sh | sh
#
# Env overrides:
#   ASK_INSTALL_DIR   target directory (default: /usr/local/bin if writable, else ~/.local/bin)

set -eu

REPO="lev-gc/ask"
BIN_NAME="ask"

if [ -t 1 ] && [ -z "${NO_COLOR:-}" ]; then
    C_RED=$(printf '\033[31m'); C_GREEN=$(printf '\033[32m')
    C_YEL=$(printf '\033[33m'); C_CYAN=$(printf '\033[36m')
    C_BOLD=$(printf '\033[1m'); C_RESET=$(printf '\033[0m')
else
    C_RED=; C_GREEN=; C_YEL=; C_CYAN=; C_BOLD=; C_RESET=
fi

info() { printf '%s==>%s %s\n' "$C_CYAN" "$C_RESET" "$*"; }
warn() { printf '%swarn:%s %s\n' "$C_YEL" "$C_RESET" "$*" >&2; }
die()  { printf '%serror:%s %s\n' "$C_RED" "$C_RESET" "$*" >&2; exit 1; }

uname_s=$(uname -s)
uname_m=$(uname -m)

case "$uname_s" in
    Linux)  os=linux ;;
    Darwin) os=macos ;;
    *)      die "Unsupported OS: $uname_s. ask supports Linux and macOS." ;;
esac

case "$uname_m" in
    x86_64|amd64)  arch=amd64 ;;
    arm64|aarch64) arch=arm64 ;;
    *)             die "Unsupported architecture: $uname_m." ;;
esac

asset="ask-${os}-${arch}"

# Keep this list in sync with .github/workflows/release.yml
case "$asset" in
    ask-linux-amd64|ask-macos-amd64|ask-macos-arm64) ;;
    *)
        die "No prebuilt binary for ${os}/${arch}. Build from source: https://github.com/${REPO}/blob/main/DEVELOPMENT.md" ;;
esac

url="https://github.com/${REPO}/releases/latest/download/${asset}"

if [ -n "${ASK_INSTALL_DIR:-}" ]; then
    install_dir="$ASK_INSTALL_DIR"
elif [ -w /usr/local/bin ]; then
    install_dir=/usr/local/bin
else
    install_dir="$HOME/.local/bin"
fi

mkdir -p "$install_dir" || die "Cannot create $install_dir"
[ -w "$install_dir" ] || die "$install_dir is not writable. Rerun with sudo, or set ASK_INSTALL_DIR=\$HOME/.local/bin"

if command -v curl >/dev/null 2>&1; then
    fetcher=curl
elif command -v wget >/dev/null 2>&1; then
    fetcher=wget
else
    die "Need curl or wget."
fi

tmp=$(mktemp 2>/dev/null) || tmp=$(mktemp -t ask 2>/dev/null) || tmp="${TMPDIR:-/tmp}/ask.$$"
trap 'rm -f "$tmp"' EXIT INT HUP TERM

info "Detected ${C_BOLD}${os}/${arch}${C_RESET}"
info "Downloading $asset"
info "  $url"

if [ "$fetcher" = curl ]; then
    curl -fSL --proto '=https' --tlsv1.2 -o "$tmp" "$url" || die "Download failed."
else
    wget -O "$tmp" "$url" || die "Download failed."
fi

# Sanity check: a real binary should be far larger than any error/redirect page.
size=$(wc -c < "$tmp" | tr -d ' ')
if [ "$size" -lt 102400 ]; then
    die "Downloaded file is only $size bytes (expected > 100KB). The release may not exist yet at $url"
fi

chmod +x "$tmp"
dest="$install_dir/$BIN_NAME"
mv -f "$tmp" "$dest" || die "Failed to install to $dest"
trap - EXIT INT HUP TERM

printf '%sInstalled:%s %s\n' "$C_GREEN" "$C_RESET" "$dest"

case ":$PATH:" in
    *":$install_dir:"*) ;;
    *)
        warn "$install_dir is not in your PATH."
        printf '       Add to your shell config:\n'
        printf '         %sexport PATH="%s:$PATH"%s\n' "$C_BOLD" "$install_dir" "$C_RESET"
        ;;
esac

"$dest" --version 2>/dev/null || true
printf 'Run %sask --help%s to get started.\n' "$C_BOLD" "$C_RESET"
