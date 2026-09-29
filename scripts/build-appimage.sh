#!/usr/bin/env bash
# Builds dist/stitchcraft-linux-<arch>.AppImage: the editor plus the Qt libraries,
# plugins and QML modules it needs, in one file that runs on most Linux distros.
#
# linuxdeploy with its Qt plugin does the gathering. Qt has to be findable: qmake
# on PATH (install-qt-action arranges this in CI), or QMAKE set to it.
#
# Usage: scripts/build-appimage.sh [--skip-build]
# Only builds for the machine it runs on (x86_64 or aarch64).

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

ARCH="$(uname -m)"
case "$ARCH" in
  x86_64) OUT_ARCH=x86_64 ;;
  aarch64 | arm64) ARCH=aarch64; OUT_ARCH=arm64 ;;
  *) echo "error: unsupported architecture '$ARCH'" >&2; exit 1 ;;
esac

if [ "${1:-}" != "--skip-build" ]; then
  cargo build --release
fi

QMAKE_BIN="${QMAKE:-$(command -v qmake6 || command -v qmake || true)}"
if [ -z "$QMAKE_BIN" ]; then
  echo "error: qmake not found - install Qt 6 and put it on PATH, or set QMAKE" >&2
  exit 1
fi

TOOLS="$(mktemp -d)"
trap 'rm -rf "$TOOLS"' EXIT
BASE="https://github.com/linuxdeploy"
curl -fsSL -o "$TOOLS/linuxdeploy" "$BASE/linuxdeploy/releases/download/continuous/linuxdeploy-$ARCH.AppImage"
curl -fsSL -o "$TOOLS/linuxdeploy-plugin-qt" "$BASE/linuxdeploy-plugin-qt/releases/download/continuous/linuxdeploy-plugin-qt-$ARCH.AppImage"
curl -fsSL -o "$TOOLS/linuxdeploy-plugin-appimage" "$BASE/linuxdeploy-plugin-appimage/releases/download/continuous/linuxdeploy-plugin-appimage-$ARCH.AppImage"
chmod +x "$TOOLS"/*

# CI containers have no FUSE, so let the tools unpack themselves and run from there.
export APPIMAGE_EXTRACT_AND_RUN=1
# linuxdeploy finds its plugins by name on PATH.
export PATH="$TOOLS:$PATH"
export QMAKE="$QMAKE_BIN"
# The plugin scans this folder's QML for imports to know which QML modules to ship.
export QML_SOURCES_PATHS="$REPO_ROOT/crates/app/qml"
export LINUXDEPLOY_OUTPUT_VERSION="$(sed -n 's/^version = "\(.*\)"/\1/p' Cargo.toml | head -1)"

rm -rf AppDir
mkdir -p dist
"$TOOLS/linuxdeploy" \
  --appdir AppDir \
  --executable target/release/stitchcraft \
  --desktop-file res/stitchcraft.desktop \
  --icon-file res/icons/stitchcraft.png \
  --plugin qt \
  --output appimage

mv Stitchcraft*.AppImage "dist/stitchcraft-linux-$OUT_ARCH.AppImage"
echo "Built dist/stitchcraft-linux-$OUT_ARCH.AppImage"
