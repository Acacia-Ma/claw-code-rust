#!/bin/sh
# install.sh — Download and install the latest devo binary for Linux / macOS.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/7df-lab/devo/main/install.sh | sh
#
# You can pin a specific version by setting the VERSION env var:
#   VERSION=v0.1.0 curl -fsSL ... | sh

set -eu

REPO="7df-lab/devo"
DEFAULT_VERSION="latest"

# ── Platform detection ───────────────────────────────────────────────────
detect_target() {
    arch="$(uname -m)"
    os="$(uname -s)"

    case "$os" in
        Linux)  os="unknown-linux-gnu" ;;
        Darwin) os="apple-darwin" ;;
        *)
            echo "Unsupported OS: $os"
            exit 1
            ;;
    esac

    case "$arch" in
        x86_64|amd64) arch="x86_64" ;;
        aarch64|arm64) arch="aarch64" ;;
        *)
            echo "Unsupported architecture: $arch"
            exit 1
            ;;
    esac

    echo "${arch}-${os}"
}

# ── Resolve version ──────────────────────────────────────────────────────
resolve_version() {
    if [ "${VERSION:-}" != "" ]; then
        echo "$VERSION"
        return
    fi

    # Fetch the latest release tag from GitHub API (unauthenticated, rate-limited).
    latest="$(
        curl -fsSL "https://api.github.com/repos/${REPO}/releases/latest" \
            | grep '"tag_name"' \
            | sed 's/.*: "//;s/",//'
    )"

    if [ -z "$latest" ]; then
        echo "Failed to resolve latest version" >&2
        exit 1
    fi
    echo "$latest"
}

# ── Install ──────────────────────────────────────────────────────────────
main() {
    target="$(detect_target)"
    version="$(resolve_version)"
    archive_url="https://github.com/${REPO}/releases/download/${version}/devo-${version}-${target}.tar.gz"

    echo "Downloading devo ${version} for ${target}..."

    tmpdir="$(mktemp -d)"
    # shellcheck disable=SC2064
    trap "rm -rf '$tmpdir'" EXIT

    curl -fsSL "$archive_url" -o "$tmpdir/devo.tar.gz"
    tar xzf "$tmpdir/devo.tar.gz" -C "$tmpdir"

    # Determine install directory.
    if [ -w /usr/local/bin ]; then
        install_dir="/usr/local/bin"
    elif [ -w "$HOME/.local/bin" ]; then
        install_dir="$HOME/.local/bin"
    else
        install_dir="$HOME/.cargo/bin"
    fi
    mkdir -p "$install_dir"

    # The archive contains a top-level directory like devo-v0.1.0-x86_64-unknown-linux-gnu/devo
    bin_src="$(find "$tmpdir" -name 'devo' -type f | head -1)"
    install -m 755 "$bin_src" "$install_dir/devo"

    echo "Installed devo to ${install_dir}/devo"
    echo "Make sure ${install_dir} is in your PATH."
    echo "Run 'devo onboard' to get started."
}

main
