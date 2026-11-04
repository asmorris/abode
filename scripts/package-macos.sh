#!/usr/bin/env bash
# Build a universal (Apple Silicon + Intel) Abode.app and package it as
#   dist/Abode-<version>-mac.dmg   (for people: drag to Applications)
#   dist/Abode-<version>-mac.zip   (for the in-app updater)
#
# Usage: scripts/package-macos.sh [version]   (defaults to the Cargo version)
set -euo pipefail
cd "$(dirname "$0")/.."

VERSION="${1:-$(grep -m1 '^version' Cargo.toml | sed -E 's/.*"(.*)".*/\1/')}"
VERSION="${VERSION#v}"
DIST=dist
APP="$DIST/Abode.app"

echo "==> Building Abode $VERSION (universal)"
# Cross-compiling for Intel needs rustup's toolchain (Homebrew's Rust can't add
# targets), so put it first on PATH if both are installed.
if command -v rustup >/dev/null; then
  export PATH="$(dirname "$(rustup which cargo)"):$PATH"
  rustup target add aarch64-apple-darwin x86_64-apple-darwin >/dev/null
fi
export MACOSX_DEPLOYMENT_TARGET=11.0
cargo build --release --locked -p abode-app --target aarch64-apple-darwin
cargo build --release --locked -p abode-app --target x86_64-apple-darwin

echo "==> Assembling $APP"
rm -rf "$DIST"
mkdir -p "$APP/Contents/MacOS" "$APP/Contents/Resources"
lipo -create \
  target/aarch64-apple-darwin/release/abode \
  target/x86_64-apple-darwin/release/abode \
  -output "$APP/Contents/MacOS/abode"
sed "s/__VERSION__/$VERSION/g" packaging/macos/Info.plist > "$APP/Contents/Info.plist"
cp crates/abode-app/assets/icon/icon.icns "$APP/Contents/Resources/AppIcon.icns"
printf 'APPL????' > "$APP/Contents/PkgInfo"

# Ad-hoc signature: required for Apple Silicon. A real Developer ID would go
# here (codesign --sign "Developer ID Application: …" --options runtime) plus
# notarisation, if we ever pay for one.
codesign --force --deep --sign - "$APP"

echo "==> Zipping for the updater"
ditto -c -k --keepParent "$APP" "$DIST/Abode-$VERSION-mac.zip"

echo "==> Making the disk image"
STAGE="$DIST/dmg"
mkdir -p "$STAGE"
cp -R "$APP" "$STAGE/"
ln -s /Applications "$STAGE/Applications"
hdiutil create -volname "Abode" -srcfolder "$STAGE" -ov -format UDZO "$DIST/Abode-$VERSION-mac.dmg" >/dev/null
rm -rf "$STAGE"

echo "==> Done:"
ls -lh "$DIST"/Abode-"$VERSION"-mac.*
