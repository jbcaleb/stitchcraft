$ErrorActionPreference = 'Stop'

# A running editor keeps its files locked, so stop it first.
Get-Process -Name 'stitchcraft' -ErrorAction SilentlyContinue | Stop-Process -Force

[array]$keys = Get-UninstallRegistryKey -SoftwareName 'Stitchcraft*'
if ($keys.Count -eq 1) {
  $packageArgs = @{
    packageName    = 'stitchcraft'
    fileType       = 'exe'
    silentArgs     = '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART'
    file           = $keys[0].UninstallString -replace '^"?([^"]+)"?.*$', '$1'
    validExitCodes = @(0)
  }
  Uninstall-ChocolateyPackage @packageArgs
} elseif ($keys.Count -eq 0) {
  Write-Warning 'Stitchcraft is not installed, nothing to uninstall.'
} else {
  throw "Found $($keys.Count) matching uninstall entries, refusing to guess."
}
