# Shared helpers for the Windows packaging scripts.
# Dot-source it:  . "$PSScriptRoot\common.ps1"
#
# Written to run under both Windows PowerShell 5.1 and PowerShell 7 - no `&&`,
# no ternaries, no `??` - because `just` uses whichever the machine has.

$ErrorActionPreference = "Stop"
$RepoRoot = Split-Path -Parent $PSScriptRoot

# The version in the workspace manifest, e.g. "0.1.0".
function Get-AppVersion {
    $toml = Get-Content (Join-Path $RepoRoot "Cargo.toml") -Raw
    if ($toml -match '(?m)^version\s*=\s*"([^"]+)"') { return $Matches[1] }
    return "0.0.0-dev"
}

# The folder holding qmake.exe, windeployqt.exe and friends.
#
# Looked up in the order a developer would expect to win:
#   1. $env:QMAKE - what CI and an interactive shell set;
#   2. .cargo/config.toml - where a local checkout keeps it, and which only Cargo
#      itself reads, so a script run from `just` has to look for it by hand;
#   3. whatever `qmake` is on PATH - what install-qt-action arranges in CI.
function Find-QtBin {
    if ($env:QMAKE -and (Test-Path $env:QMAKE)) { return Split-Path -Parent $env:QMAKE }

    $config = Join-Path $RepoRoot ".cargo/config.toml"
    if (Test-Path $config) {
        $text = Get-Content $config -Raw
        if ($text -match "(?m)^\s*QMAKE\s*=\s*['""]([^'""]+)['""]") {
            if (Test-Path $Matches[1]) { return Split-Path -Parent $Matches[1] }
        }
    }

    $onPath = Get-Command qmake.exe -ErrorAction SilentlyContinue
    if ($onPath) { return Split-Path -Parent $onPath.Source }

    throw "Cannot find Qt. Set QMAKE, or copy .cargo/config.toml.example to .cargo/config.toml and point it at your Qt's qmake.exe."
}

# The Visual C++ install folder (...\VC\), where the redistributable runtime lives.
#
# windeployqt finds the runtime DLLs through VCINSTALLDIR, which only a Visual
# Studio developer prompt sets - so from an ordinary shell it copies nothing and
# only prints a warning. vswhere is Microsoft's own locator; it ships with the
# Visual Studio installer, including the Build Tools.
function Find-VCInstallDir {
    if ($env:VCINSTALLDIR -and (Test-Path $env:VCINSTALLDIR)) { return $env:VCINSTALLDIR }

    $vswhere = Join-Path ${env:ProgramFiles(x86)} "Microsoft Visual Studio\Installer\vswhere.exe"
    if (Test-Path $vswhere) {
        $root = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
        if ($root) {
            $vc = Join-Path ($root | Select-Object -First 1) "VC"
            if (Test-Path $vc) { return ($vc + "\") }
        }
    }
    return $null
}

# Copies the Visual C++ runtime DLLs (vcruntime140.dll, msvcp140.dll, ...) into
# $Dest, so the app runs on a PC that has never had Visual C++ installed.
#
# Done by hand instead of with windeployqt --compiler-runtime, which copied
# nothing here and said nothing: it appears to pick the alphabetically last folder
# under Redist\MSVC, which is `v143` (an installer stub with no DLLs) rather than
# the versioned folder (14.44.35112) that has them. Picking the newest *numeric*
# version is deterministic.
function Copy-CRuntime([string]$Dest, [string]$Arch) {
    $vc = Find-VCInstallDir
    if (-not $vc) { throw "Visual C++ not found, so the runtime DLLs cannot be staged. Install the 'MSVC v143 build tools'." }

    $redist = Join-Path $vc "Redist\MSVC"
    $version = Get-ChildItem $redist -Directory -ErrorAction SilentlyContinue |
        Where-Object { $_.Name -match '^\d+\.\d+\.\d+' } |
        Sort-Object { [version]$_.Name } -Descending | Select-Object -First 1
    if (-not $version) { throw "No versioned redistributable folder under $redist." }

    $folder = if ($Arch -eq "arm64") { "arm64" } else { "x64" }
    $crt = Get-ChildItem (Join-Path $version.FullName "$folder\Microsoft.VC*.CRT") -Directory -ErrorAction SilentlyContinue |
        Select-Object -First 1
    if (-not $crt) { throw "No $folder runtime under $($version.FullName)." }

    Copy-Item (Join-Path $crt.FullName "*.dll") $Dest -Force
    Write-Host "Copied the Visual C++ runtime from $($crt.FullName)"
}

# Inno Setup's compiler: on PATH, or where its installer puts it (per machine or per user).
function Find-InnoSetup {
    $onPath = Get-Command ISCC.exe -ErrorAction SilentlyContinue
    if ($onPath) { return $onPath.Source }
    $candidates = @(
        (Join-Path ${env:ProgramFiles(x86)} "Inno Setup 6\ISCC.exe"),
        (Join-Path $env:ProgramFiles "Inno Setup 6\ISCC.exe"),
        (Join-Path $env:LOCALAPPDATA "Programs\Inno Setup 6\ISCC.exe")
    )
    foreach ($c in $candidates) { if ($c -and (Test-Path $c)) { return $c } }
    return $null
}

# The Windows SDK tool (makeappx.exe, signtool.exe): newest installed version.
function Find-SdkTool([string]$Name) {
    $root = Join-Path ${env:ProgramFiles(x86)} "Windows Kits\10\bin"
    $tool = Get-ChildItem "$root\*\x64\$Name" -ErrorAction SilentlyContinue |
        Sort-Object FullName -Descending | Select-Object -First 1
    if (-not $tool) { throw "$Name not found (install the Windows SDK)." }
    return $tool.FullName
}

# The release build directory for a target triple, or for the host when none is given.
function Get-ReleaseDir([string]$Target) {
    if ($Target) { return Join-Path $RepoRoot "target/$Target/release" }
    return Join-Path $RepoRoot "target/release"
}

# x86_64-pc-windows-msvc -> x86_64 ; aarch64-pc-windows-msvc -> arm64
function Get-ArchName([string]$Target) {
    if ($Target -like "aarch64*") { return "arm64" }
    return "x86_64"
}
