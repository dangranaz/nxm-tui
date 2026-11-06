#!/bin/sh
# nxm-tui installer.
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/dangranaz/nxm-tui/main/install.sh | sh
#
# Env overrides:
#   NXM_INSTALL_DIR   install directory (default: $HOME/.local/bin)
#   NXM_VERSION       release tag to install (default: latest)
#
# Supported platforms: Apple Silicon macOS, Linux x86_64.
set -eu

REPO="dangranaz/nxm-tui"
BIN="nxm-tui"
INSTALL_DIR="${NXM_INSTALL_DIR:-$HOME/.local/bin}"
VERSION="${NXM_VERSION:-latest}"

err() {
	printf 'error: %s\n' "$1" >&2
	exit 1
}

info() {
	printf '==> %s\n' "$1"
}

need() {
	command -v "$1" >/dev/null 2>&1 || err "required command not found: $1"
}

need uname
need tar
need mkdir
need install

# Pick a downloader.
if command -v curl >/dev/null 2>&1; then
	dl() { curl -fsSL "$1" -o "$2"; }
	dl_stdout() { curl -fsSL "$1"; }
elif command -v wget >/dev/null 2>&1; then
	dl() { wget -qO "$2" "$1"; }
	dl_stdout() { wget -qO- "$1"; }
else
	err "need curl or wget"
fi

# Detect OS + architecture and map to a release target triple.
os="$(uname -s)"
arch="$(uname -m)"
case "$os" in
Darwin)
	case "$arch" in
	arm64 | aarch64) target="aarch64-apple-darwin" ;;
	*) err "unsupported macOS architecture: $arch (only Apple Silicon is built)" ;;
	esac
	;;
Linux)
	case "$arch" in
	x86_64 | amd64) target="x86_64-unknown-linux-gnu" ;;
	*) err "unsupported Linux architecture: $arch (only x86_64 is built)" ;;
	esac
	;;
*)
	err "unsupported OS: $os"
	;;
esac
info "detected platform: $target"

# Resolve the release tag.
if [ "$VERSION" = "latest" ]; then
	info "resolving latest release"
	tag="$(dl_stdout "https://api.github.com/repos/${REPO}/releases/latest" |
		grep '"tag_name"' | head -1 | cut -d '"' -f4)"
	[ -n "$tag" ] || err "could not resolve latest release tag"
else
	tag="$VERSION"
fi
info "installing $BIN $tag"

staging="${BIN}-${tag}-${target}"
tarball="${staging}.tar.gz"
base_url="https://github.com/${REPO}/releases/download/${tag}"

tmp="$(mktemp -d)"
trap 'rm -rf "$tmp"' EXIT INT TERM

info "downloading ${tarball}"
dl "${base_url}/${tarball}" "${tmp}/${tarball}" || err "download failed: ${base_url}/${tarball}"

# Verify checksum when a sha256 tool is available and the asset exists.
if dl "${base_url}/${tarball}.sha256" "${tmp}/${tarball}.sha256" 2>/dev/null; then
	info "verifying checksum"
	expected="$(cut -d ' ' -f1 <"${tmp}/${tarball}.sha256")"
	if command -v shasum >/dev/null 2>&1; then
		actual="$(shasum -a 256 "${tmp}/${tarball}" | cut -d ' ' -f1)"
	elif command -v sha256sum >/dev/null 2>&1; then
		actual="$(sha256sum "${tmp}/${tarball}" | cut -d ' ' -f1)"
	else
		actual=""
	fi
	if [ -n "$actual" ] && [ "$expected" != "$actual" ]; then
		err "checksum mismatch (expected $expected, got $actual)"
	fi
else
	info "no checksum published; skipping verification"
fi

info "extracting"
tar xzf "${tmp}/${tarball}" -C "$tmp"

mkdir -p "$INSTALL_DIR"
install -m 0755 "${tmp}/${staging}/${BIN}" "${INSTALL_DIR}/${BIN}"
info "installed to ${INSTALL_DIR}/${BIN}"

# PATH hint (the script does not edit shell rc files).
case ":${PATH}:" in
*":${INSTALL_DIR}:"*) ;;
*)
	printf '\n'
	info "note: ${INSTALL_DIR} is not on your PATH."
	info "add this line to your ~/.zshrc or ~/.bashrc:"
	# shellcheck disable=SC2016  # $PATH is intentionally literal for the user to paste.
	printf '\n    export PATH="%s:$PATH"\n\n' "$INSTALL_DIR"
	;;
esac

info "done — run: ${BIN}"
