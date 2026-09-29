<#
.SYNOPSIS
    Assembles a self-contained Windows folder: stitchcraft.exe plus every Qt file
    it needs, ready for the installer, the MSIX, or a straight copy.

.DESCRIPTION
    The release exe links Qt dynamically, so on its own it only runs on a machine
    with Qt on PATH. windeployqt reads the exe and the QML it loads and copies in
    the right Qt DLLs, platform plugins and QML modules, and the C++ runtime.

    Expects the release build to exist already (`cargo build --release`).

.PARAMETER Target
    Target triple the exe was built for. Empty means the host's default, i.e.
    target/release.

.PARAMETER Out
    Where to assemble the folder. Emptied first. Defaults to dist/stage.

.EXAMPLE
    pwsh -File scripts/stage-windows.ps1
    pwsh -File scripts/stage-windows.ps1 -Target aarch64-pc-windows-msvc
#>
param(
    [string]$Target = "",
    [string]$Out = ""
)

. "$PSScriptRoot\common.ps1"

if (-not $Out) { $Out = Join-Path $RepoRoot "dist/stage" }
$exe = Join-Path (Get-ReleaseDir $Target) "stitchcraft.exe"
if (-not (Test-Path $exe)) { throw "No release build at $exe - run 'cargo build --release' first." }

$qtBin = Find-QtBin
$windeployqt = Join-Path $qtBin "windeployqt.exe"
if (-not (Test-Path $windeployqt)) { throw "windeployqt.exe not found in $qtBin" }

if (Test-Path $Out) { Remove-Item $Out -Recurse -Force }
New-Item -ItemType Directory -Force -Path $Out | Out-Null
Copy-Item $exe $Out

# --qmldir makes it scan the app's QML for `import`s, which is how it knows to
# bring QtQuick.Controls, Layouts, Shapes and Dialogs. The QML itself is compiled
# into the exe, so this is the only way it can learn what the app imports.
#
# --no-compiler-runtime because Copy-CRuntime below does that job reliably.
# --skip-plugin-types drops the QML debugger plugins (never wanted in a release)
# and the TUIO touch-input plugin.
Write-Host "Running windeployqt ($qtBin)..."
& $windeployqt --release --no-compiler-runtime --no-translations --no-system-d3d-compiler --no-opengl-sw `
    --skip-plugin-types qmltooling,generic `
    --qmldir (Join-Path $RepoRoot "crates/app/qml") (Join-Path $Out "stitchcraft.exe")
if ($LASTEXITCODE -ne 0) { throw "windeployqt failed" }

# The app pins Qt Quick Controls to the Basic style (crates/app/qml/
# qtquickcontrols2.conf), and Fusion is the fallback if that ever fails to load.
# The other styles are several megabytes each that nothing will ever ask for: their
# QML folders under qml/, and the matching Qt6QuickControls2<Style>*.dll beside the exe.
$styles = Join-Path $Out "qml/QtQuick/Controls"
foreach ($unused in "Imagine", "Material", "Universal", "FluentWinUI3", "Windows") {
    $dir = Join-Path $styles $unused
    if (Test-Path $dir) { Remove-Item $dir -Recurse -Force }
    Get-ChildItem $Out -Filter "Qt6QuickControls2$unused*.dll" | Remove-Item -Force
}

# The exe and Qt both need the Visual C++ runtime, and a clean Windows install does
# not have it: without these DLLs the app would start here and fail on a fresh PC
# with an error about a missing DLL.
Copy-CRuntime -Dest $Out -Arch (Get-ArchName $Target)
foreach ($dll in "vcruntime140.dll", "msvcp140.dll") {
    if (-not (Test-Path (Join-Path $Out $dll))) { throw "$dll was not staged - the app would not start on a machine without Visual C++ installed." }
}

# The GPL asks for a copy of the license to travel with the program.
Copy-Item (Join-Path $RepoRoot "LICENSE") $Out

$size = (Get-ChildItem $Out -Recurse -File | Measure-Object Length -Sum).Sum
Write-Host ("Staged {0} files, {1:N1} MB, in {2}" -f (Get-ChildItem $Out -Recurse -File).Count, ($size / 1MB), $Out)
