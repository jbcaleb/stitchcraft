#!/usr/bin/env bash
# Builds dist/Stitchcraft.app: a macOS app bundle with the Qt frameworks and QML
# modules the editor needs, ad-hoc code-signed.
#
# The binary links Qt dynamically. macdeployqt copies the frameworks in and
# rewrites the binary to find them inside the bundle, so the .app runs on a Mac
# that has no Qt installed.
#
# Qt has to be findable: qmake on PATH (what install-qt-action arranges), or QMAKE
# set to it.
#
# Usage: scripts/build-macos-bundle.sh [target-triple]
# With no argument, builds for the host.

set -euo pipefail

TARGET="${1:-}"
if [ -n "$TARGET" ]; then
  case "$TARGET" in
    aarch64-apple-darwin | x86_64-apple-darwin) ;;
    *)
      echo "error: unsupported target '$TARGET' (expected aarch64-apple-darwin or x86_64-apple-darwin)" >&2
      exit 1
      ;;
  esac
  CARGO_TARGET_ARGS=(--target "$TARGET")
  RELEASE_DIR="target/$TARGET/release"
else
  # No triple: build for the host into plain target/release, so a prior
  # `cargo build --release` (as `just build` does) is reused, not repeated.
  CARGO_TARGET_ARGS=()
  RELEASE_DIR="target/release"
fi

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"
APP_NAME="Stitchcraft"

QMAKE_BIN="${QMAKE:-$(command -v qmake6 || command -v qmake || true)}"
if [ -z "$QMAKE_BIN" ]; then
  echo "error: qmake not found - install Qt 6 and put it on PATH, or set QMAKE" >&2
  exit 1
fi
QT_BIN="$(dirname "$QMAKE_BIN")"
MACDEPLOYQT="$QT_BIN/macdeployqt"
if [ ! -x "$MACDEPLOYQT" ]; then
  echo "error: macdeployqt not found next to $QMAKE_BIN" >&2
  exit 1
fi

echo "Building $APP_NAME (release${TARGET:+, $TARGET})..."
cargo build --release ${CARGO_TARGET_ARGS[@]+"${CARGO_TARGET_ARGS[@]}"}

DIST="dist/$APP_NAME.app"
rm -rf "$DIST"
CONTENTS="$DIST/Contents"
mkdir -p "$CONTENTS/MacOS" "$CONTENTS/Resources"

echo "Assembling bundle at $DIST..."
cp "$RELEASE_DIR/stitchcraft" "$CONTENTS/MacOS/$APP_NAME"
sed -e "s/__VERSION__/$VERSION/g" installer/macos/Info.plist.in > "$CONTENTS/Info.plist"

# icon.icns, built from the 512px PNG at every size an .icns holds.
ICONSET_PARENT="$(mktemp -d)"
ICONSET="$ICONSET_PARENT/icon.iconset"
mkdir -p "$ICONSET"
for size in 16 32 64 128 256 512; do
  sips -z "$size" "$size" res/icons/stitchcraft.png --out "$ICONSET/icon_${size}x${size}.png" >/dev/null
  double=$((size * 2))
  sips -z "$double" "$double" res/icons/stitchcraft.png --out "$ICONSET/icon_${size}x${size}@2x.png" >/dev/null
done
iconutil -c icns "$ICONSET" -o "$CONTENTS/Resources/icon.icns"
rm -rf "$ICONSET_PARENT"

# -qmldir makes it scan the app's QML for `import`s: the QML itself is compiled into
# the executable, so this is the only way it learns which QML modules to bring.
"$MACDEPLOYQT" "$DIST" -qmldir=crates/app/qml

# Ad-hoc signature ("-" is the free, local identity; no Developer Program needed).
# It is not optional: Apple Silicon refuses to run a binary with no signature at
# all, and macdeployqt's edits invalidate the one the linker gave it. Gatekeeper
# still shows its "unidentified developer" prompt on first launch - right-click >
# Open - because the app is not notarized.
codesign --force --deep --sign - "$DIST"

echo "Built $DIST"
