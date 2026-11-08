#!/usr/bin/env bash
# Build Abode for this Linux machine's architecture and package it as
#   dist/Abode-<version>-linux-<arch>.tar.gz
# (for people: unpack and run install.sh; also what the in-app updater uses).
#
# The binary needs at least the glibc of the machine it's built on, so release
# builds run on an older Ubuntu (see .github/workflows/release.yml).
#
# Usage: scripts/package-linux.sh [version]   (defaults to the Cargo version)
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION="${1:-$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"(.*)".*/\1/')}"
VERSION="${VERSION#v}"
ARCH=$(uname -m)   # x86_64 or aarch64, the names the updater looks for
DIST=dist
TARGET_DIR="${CARGO_TARGET_DIR:-target}"

echo "==> Building Abode $VERSION ($ARCH)"
cargo build --release --locked -p abode-app

echo "==> Assembling the app folder"
STAGE="$DIST/linux"
rm -rf "$STAGE"
mkdir -p "$STAGE/Abode"
install -m 755 "$TARGET_DIR/release/abode" "$STAGE/Abode/abode"
install -m 755 packaging/linux/install.sh "$STAGE/Abode/install.sh"
install -m 644 packaging/linux/abode.desktop "$STAGE/Abode/abode.desktop"
install -m 644 packaging/linux/abode-mime.xml "$STAGE/Abode/abode-mime.xml"
install -m 644 crates/abode-app/assets/icon/icon-512.png "$STAGE/Abode/abode.png"

OUT="$DIST/Abode-$VERSION-linux-$ARCH.tar.gz"
tar -czf "$OUT" -C "$STAGE" Abode
rm -rf "$STAGE"

echo "==> Done:"
ls -lh "$OUT"
