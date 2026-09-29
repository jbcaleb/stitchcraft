<#
.SYNOPSIS
    Packs dist/stitchcraft-windows-<arch>.msix from an existing release build.

.DESCRIPTION
    Stages the exe with Qt, adds store logos resized from the app icon and the
    manifest, and runs makeappx. The package is left unsigned; sign-msix-test.ps1
    signs a copy with a throwaway certificate for local installs.

.EXAMPLE
    pwsh -File scripts/build-msix.ps1 -Version 0.1.0
    pwsh -File scripts/build-msix.ps1 -Version 0.1.0 -Target aarch64-pc-windows-msvc -Arch arm64
#>
param(
    [string]$Version = "",
    [string]$Target = "",
    [string]$Arch = "x64"
)

. "$PSScriptRoot\common.ps1"

if (-not $Version) { $Version = Get-AppVersion }

$stage = Join-Path $RepoRoot "dist/msix-stage"
$name = if ($Arch -eq "x64") { "x86_64" } else { $Arch }
$out = Join-Path $RepoRoot "dist/stitchcraft-windows-$name.msix"

# MSIX versions are a.b.c.d and the Store reserves the last part.
if ($Version -notmatch '^v?(\d+)\.(\d+)\.(\d+)') { throw "Bad version '$Version'" }
$msixVersion = "$($Matches[1]).$($Matches[2]).$($Matches[3]).0"

& "$PSScriptRoot\stage-windows.ps1" -Target $Target -Out $stage
New-Item -ItemType Directory -Force -Path "$stage/Assets" | Out-Null

# Store logos, resized from the app icon.
Add-Type -AssemblyName System.Drawing
$icon = [System.Drawing.Image]::FromFile((Join-Path $RepoRoot "res/icons/stitchcraft.png"))
$logos = @{ "StoreLogo" = 50; "Square44x44Logo" = 44; "Square150x150Logo" = 150 }
foreach ($logo in $logos.GetEnumerator()) {
    $size = $logo.Value
    $bmp = New-Object System.Drawing.Bitmap $size, $size
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
    $g.DrawImage($icon, 0, 0, $size, $size)
    $g.Dispose()
    $bmp.Save((Join-Path $stage "Assets/$($logo.Key).png"), [System.Drawing.Imaging.ImageFormat]::Png)
    $bmp.Dispose()
}
$icon.Dispose()

$manifest = (Get-Content (Join-Path $RepoRoot "packaging/msix/AppxManifest.xml") -Raw).
    Replace("__VERSION__", $msixVersion).
    Replace("__ARCH__", $Arch)
[System.IO.File]::WriteAllText((Join-Path $stage "AppxManifest.xml"), $manifest, (New-Object System.Text.UTF8Encoding($false)))

$makeappx = Find-SdkTool "makeappx.exe"
& $makeappx pack /d $stage /p $out /o
if ($LASTEXITCODE -ne 0) { throw "makeappx failed" }
Write-Host "Built $out"
