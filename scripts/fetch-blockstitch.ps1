# Refreshes vendor/blockstitch from upstream. Run from the repository root.
#
# The qml branch carries both the Vue frontend (src/) and the Qt 6 one
# (qml/ + rust/), plus crates/blockstitch-core. Stitchcraft uses the Qt half.
param([string]$Branch = "qml")

$ErrorActionPreference = "Stop"
$target = Join-Path $PSScriptRoot "..\vendor\blockstitch"

if (Test-Path $target) { Remove-Item -Recurse -Force $target }

# core.longpaths keeps the clone working under a deep checkout path.
git -c core.longpaths=true clone --depth 1 --branch $Branch `
    https://github.com/Blockworked/blockstitch.git $target

Remove-Item -Recurse -Force (Join-Path $target ".git")
Write-Host "vendor/blockstitch updated from branch '$Branch'."
