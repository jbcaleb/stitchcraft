<#
.SYNOPSIS
    Builds Stitchcraft for Windows and packages it into an installer.

.DESCRIPTION
    Builds the release exe, stages it with Qt (stage-windows.ps1), then compiles
    installer/stitchcraft.iss with Inno Setup 6 (https://jrsoftware.org/isdl.php).
    Writes dist/stitchcraft-windows-<arch>-setup.exe.

.PARAMETER Version
    Version stamped on the installer. Defaults to the one in Cargo.toml.

.PARAMETER Target
    Target triple. Defaults to the host's.

.PARAMETER SkipBuild
    Reuse the existing release build and just stage and package it.

.EXAMPLE
    pwsh -File scripts/build-installer.ps1
    pwsh -File scripts/build-installer.ps1 -Version 0.2.0
#>
param(
    [string]$Version = "",
    [string]$Target = "",
    [switch]$SkipBuild
)

. "$PSScriptRoot\common.ps1"

if (-not $Version) { $Version = Get-AppVersion }
$arch = Get-ArchName $Target
$installerArch = if ($arch -eq "arm64") { "arm64" } else { "x64compatible" }

$iscc = Find-InnoSetup
if (-not $iscc) { throw "ISCC.exe not found. Install Inno Setup 6: https://jrsoftware.org/isdl.php" }

Push-Location $RepoRoot
try {
    if (-not $SkipBuild) {
        Write-Host "Building stitchcraft.exe (release, version $Version)..."
        if ($Target) { cargo build --release --target $Target } else { cargo build --release }
        if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }
    }

    & "$PSScriptRoot\stage-windows.ps1" -Target $Target
    if ($LASTEXITCODE -ne 0) { throw "staging failed" }

    $base = "stitchcraft-windows-$arch-setup"
    Write-Host "Compiling installer (version $Version)..."
    & $iscc "/DMyAppVersion=$Version" "/DInstallerArch=$installerArch" "/DOutputBaseFilename=$base" (Join-Path $RepoRoot "installer\stitchcraft.iss")
    if ($LASTEXITCODE -ne 0) { throw "ISCC.exe failed" }

    Write-Host "Installer written to dist\$base.exe"
} finally {
    Pop-Location
}
