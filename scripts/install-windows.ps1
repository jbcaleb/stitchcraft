<#
.SYNOPSIS
    Installs or uninstalls Stitchcraft for the current user (what `just install`
    runs on Windows).

.DESCRIPTION
    install    stages the release build (exe + Qt) into the install folder, adds a
               Start menu shortcut, and registers the app under "Installed apps"
               so Windows can uninstall it.
    uninstall  undoes all of that.

    Per-user: no administrator rights and no UAC prompt. It refuses to touch an
    install whose app is running, rather than closing someone's editor (and any
    unsaved canvas) under them.

    The three path/key parameters exist so the script can be tried against a
    scratch location without touching the real install.

.EXAMPLE
    pwsh -File scripts/install-windows.ps1 install
    pwsh -File scripts/install-windows.ps1 uninstall
#>
param(
    [Parameter(Mandatory, Position = 0)][ValidateSet("install", "uninstall")][string]$Action,
    [string]$InstallDir = (Join-Path $env:LOCALAPPDATA "Programs\Stitchcraft"),
    [string]$StartMenuDir = (Join-Path $env:APPDATA "Microsoft\Windows\Start Menu\Programs"),
    [string]$AppKey = "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\Stitchcraft"
)

. "$PSScriptRoot\common.ps1"

$shortcut = Join-Path $StartMenuDir "Stitchcraft.lnk"
$exe = Join-Path $InstallDir "stitchcraft.exe"

# True when a stitchcraft.exe from $InstallDir is running.
function Test-InstalledRunning {
    $full = [System.IO.Path]::GetFullPath($exe)
    foreach ($p in Get-Process -Name stitchcraft -ErrorAction SilentlyContinue) {
        try { if ($p.Path -and ([System.IO.Path]::GetFullPath($p.Path) -ieq $full)) { return $true } } catch { }
    }
    return $false
}

function Assert-NotRunning {
    if (Test-InstalledRunning) {
        throw "Stitchcraft is running from $InstallDir. Close it first (this script will not close it for you), then try again."
    }
}

function Install-App {
    Assert-NotRunning

    $stage = Join-Path $RepoRoot "dist/stage"
    & "$PSScriptRoot\stage-windows.ps1" -Out $stage
    if (-not (Test-Path (Join-Path $stage "stitchcraft.exe"))) { throw "Staging produced no stitchcraft.exe" }

    # Replace the folder wholesale so files dropped by a newer Qt do not linger.
    if (Test-Path $InstallDir) { Remove-Item $InstallDir -Recurse -Force }
    New-Item -ItemType Directory -Force -Path $InstallDir | Out-Null
    Copy-Item (Join-Path $stage "*") $InstallDir -Recurse -Force

    New-Item -ItemType Directory -Force -Path $StartMenuDir | Out-Null
    $shell = New-Object -ComObject WScript.Shell
    $link = $shell.CreateShortcut($shortcut)
    $link.TargetPath = $exe
    $link.WorkingDirectory = $InstallDir
    $link.Description = "Build Minecraft mods with blocks"
    $link.Save()

    # The uninstaller is this very script, copied out of the repo so removal keeps
    # working after the checkout is deleted.
    $support = Join-Path $InstallDir "uninstall"
    New-Item -ItemType Directory -Force -Path $support | Out-Null
    Copy-Item "$PSScriptRoot\install-windows.ps1", "$PSScriptRoot\common.ps1" $support -Force
    $uninstall = "powershell.exe -NoProfile -ExecutionPolicy Bypass -File `"$support\install-windows.ps1`" uninstall -InstallDir `"$InstallDir`" -StartMenuDir `"$StartMenuDir`" -AppKey `"$AppKey`""

    $size = (Get-ChildItem $InstallDir -Recurse -File | Measure-Object Length -Sum).Sum
    New-Item -Path $AppKey -Force | Out-Null
    $values = @{
        DisplayName     = "Stitchcraft"
        DisplayVersion  = (Get-AppVersion)
        Publisher       = "jbcaleb"
        InstallLocation = $InstallDir
        DisplayIcon     = $exe
        UninstallString = $uninstall
        URLInfoAbout    = "https://github.com/jbcaleb/stitchcraft"
    }
    foreach ($k in $values.Keys) { Set-ItemProperty -Path $AppKey -Name $k -Value $values[$k] }
    Set-ItemProperty -Path $AppKey -Name EstimatedSize -Value ([int]($size / 1KB)) -Type DWord
    Set-ItemProperty -Path $AppKey -Name NoModify -Value 1 -Type DWord
    Set-ItemProperty -Path $AppKey -Name NoRepair -Value 1 -Type DWord

    Write-Host "Installed Stitchcraft to $InstallDir"
}

function Uninstall-App {
    Assert-NotRunning

    if (Test-Path $shortcut) { Remove-Item $shortcut -Force }
    if (Test-Path $AppKey) { Remove-Item $AppKey -Recurse -Force }

    if (Test-Path $InstallDir) {
        # When run from the installed copy, this script is inside the folder it is
        # deleting. Windows will not delete a running script's own file, so hand the
        # deletion to a detached cmd that outlives this process.
        $here = $PSScriptRoot
        if ($here.StartsWith($InstallDir, [System.StringComparison]::OrdinalIgnoreCase)) {
            $q = '"' + $InstallDir + '"'
            Start-Process -WindowStyle Hidden -FilePath "cmd.exe" -ArgumentList "/c ping -n 3 127.0.0.1 >nul & rmdir /s /q $q"
        } else {
            Remove-Item $InstallDir -Recurse -Force
        }
    }
    Write-Host "Uninstalled Stitchcraft"
}

if ($Action -eq "install") { Install-App } else { Uninstall-App }
