#!/bin/sh
# Install Vey.
#
#   curl -fsSL https://veylang.tech/install.sh | sh
#
# The binary is put in ~/.local/bin. Another folder may be named with
# VEY_PREFIX, and another version with VEY_VERSION.

set -eu

VERSION="${VEY_VERSION:-latest}"
PREFIX="${VEY_PREFIX:-${HOME:-/tmp}/.local}"
REPO="${VEY_REPO:-vey-lang/vey}"

say() {
  printf '%s\n' "$*"
}

die() {
  printf 'vey: %s\n' "$*" >&2
  exit 1
}

# The system the script runs on.
os="$(uname -s)"
arch="$(uname -m)"

case "$os" in
  Darwin) os="apple-darwin" ;;
  Linux) os="unknown-linux-gnu" ;;
  *) die "$os is not supported yet" ;;
esac

case "$arch" in
  x86_64 | amd64) arch="x86_64" ;;
  arm64 | aarch64) arch="aarch64" ;;
  *) die "$arch is not supported yet" ;;
esac

target="$arch-$os"

if [ "$VERSION" = "latest" ]; then
  url="https://github.com/$REPO/releases/latest/download/vey-$target.tar.gz"
else
  url="https://github.com/$REPO/releases/download/v$VERSION/vey-$VERSION-$target.tar.gz"
fi

say "Installing Vey for $target..."

folder="$(mktemp -d)"
trap 'rm -rf "$folder"' EXIT

if command -v curl >/dev/null 2>&1; then
  curl -fsSL "$url" -o "$folder/vey.tar.gz" || die "the download failed: $url"
elif command -v wget >/dev/null 2>&1; then
  wget -q "$url" -O "$folder/vey.tar.gz" || die "the download failed: $url"
else
  die "curl or wget is needed to download Vey"
fi

tar -xzf "$folder/vey.tar.gz" -C "$folder" || die "the archive could not be read"

mkdir -p "$PREFIX/bin"
mv "$folder/vey" "$PREFIX/bin/vey"
chmod +x "$PREFIX/bin/vey"

say "Vey is in $PREFIX/bin/vey."

case ":$PATH:" in
  *":$PREFIX/bin:"*) ;;
  *)
    say ""
    say "That folder is not in your path yet. Add it with:"
    say ""
    say "  export PATH=\"$PREFIX/bin:\$PATH\""
    say ""
    ;;
esac

say "Try it with:"
say ""
say "  vey new myapp"
say "  cd myapp"
say "  vey dev"
