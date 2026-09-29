# Stitchcraft tasks. Needs `just` (cargo install just) and a Rust toolchain plus Qt 6
# (see README.md). Works on Windows, Linux and macOS.
#
#   just install    build, then install to this machine
#   just replace    rebuild and swap the installed copy for the new build
#   just uninstall  remove it
#
# Windows: the recipes run in PowerShell.
set windows-shell := ["powershell.exe", "-NoLogo", "-NoProfile", "-ExecutionPolicy", "Bypass", "-Command"]

# Linux install root. Override with e.g.  PREFIX=~/.local just install
prefix := env("PREFIX", "/usr/local")
mac_app := "/Applications/Stitchcraft.app"

default: build

# Optimised build of the editor.
build *args:
    cargo build --release {{args}}

# Build and start the editor.
run *args:
    cargo run --release {{args}}

test:
    cargo test --workspace

clean:
    cargo clean

# --- install / uninstall / replace ---------------------------------------------
#
# `replace` builds FIRST and only then removes the old copy, so a build that fails
# leaves the installed app alone. `install` also depends on `build`; `just` runs a
# recipe once per invocation, so that is not a second build.

# Windows: per user (%LOCALAPPDATA%\Programs\Stitchcraft), no admin rights needed.
[windows]
install: build
    powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File scripts/install-windows.ps1 install

[windows]
uninstall:
    powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File scripts/install-windows.ps1 uninstall

# Linux: binary, launcher entry and icon under `prefix` (uses sudo unless root).
# It links the system's Qt 6, so that must be installed (see README.md); for a
# self-contained build use `just appimage`.
[linux]
install: build
    #!/usr/bin/env bash
    set -euo pipefail
    SUDO=""
    if [ "$(id -u)" -ne 0 ] && [ ! -w "{{prefix}}" ]; then SUDO=sudo; fi
    $SUDO install -Dm0755 target/release/stitchcraft "{{prefix}}/bin/stitchcraft"
    $SUDO install -Dm0644 res/stitchcraft.desktop "{{prefix}}/share/applications/stitchcraft.desktop"
    $SUDO install -Dm0644 res/icons/stitchcraft.png "{{prefix}}/share/icons/hicolor/512x512/apps/stitchcraft.png"
    $SUDO install -Dm0644 res/io.github.jbcaleb.Stitchcraft.metainfo.xml "{{prefix}}/share/metainfo/io.github.jbcaleb.Stitchcraft.metainfo.xml"
    command -v update-desktop-database >/dev/null && $SUDO update-desktop-database "{{prefix}}/share/applications" || true
    command -v gtk-update-icon-cache >/dev/null && $SUDO gtk-update-icon-cache -q -t "{{prefix}}/share/icons/hicolor" || true
    echo "Installed to {{prefix}}"

[linux]
uninstall:
    #!/usr/bin/env bash
    set -euo pipefail
    SUDO=""
    if [ "$(id -u)" -ne 0 ] && [ ! -w "{{prefix}}" ]; then SUDO=sudo; fi
    $SUDO rm -f "{{prefix}}/bin/stitchcraft" \
        "{{prefix}}/share/applications/stitchcraft.desktop" \
        "{{prefix}}/share/icons/hicolor/512x512/apps/stitchcraft.png" \
        "{{prefix}}/share/metainfo/io.github.jbcaleb.Stitchcraft.metainfo.xml"
    echo "Removed from {{prefix}}"

# macOS: a self-contained .app in /Applications (uses sudo if that is not writable).
[macos]
install: build
    #!/usr/bin/env bash
    set -euo pipefail
    ./scripts/build-macos-bundle.sh
    SUDO=""
    if [ ! -w /Applications ]; then SUDO=sudo; fi
    $SUDO rm -rf "{{mac_app}}"
    $SUDO ditto dist/Stitchcraft.app "{{mac_app}}"
    $SUDO xattr -dr com.apple.quarantine "{{mac_app}}" || true
    echo "Installed to {{mac_app}}"

[macos]
uninstall:
    #!/usr/bin/env bash
    set -euo pipefail
    SUDO=""
    if [ ! -w /Applications ]; then SUDO=sudo; fi
    $SUDO rm -rf "{{mac_app}}"
    echo "Removed {{mac_app}}"

# Rebuild, then replace the installed copy with the new build.
replace: build uninstall install

# --- distributable packages ----------------------------------------------------

# Windows: the Inno Setup installer -> dist/stitchcraft-windows-<arch>-setup.exe
[windows]
installer:
    powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File scripts/build-installer.ps1

# Windows: an unsigned MSIX -> dist/stitchcraft-windows-x86_64.msix
[windows]
msix: build
    powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File scripts/build-msix.ps1

# Linux: a self-contained AppImage -> dist/stitchcraft-linux-<arch>.AppImage
[linux]
appimage:
    ./scripts/build-appimage.sh

# macOS: the app bundle -> dist/Stitchcraft.app
[macos]
bundle:
    ./scripts/build-macos-bundle.sh
