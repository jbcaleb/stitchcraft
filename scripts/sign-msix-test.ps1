<#
.SYNOPSIS
    Signs an MSIX with a throwaway self-signed certificate, so it can be installed
    locally, and writes the matching .cer next to it.

.DESCRIPTION
    The certificate subject has to equal the manifest's Publisher. A real release
    would be signed with a real certificate, or by the Store; this one is only good
    for trying the package out.

.EXAMPLE
    pwsh -File scripts/sign-msix-test.ps1 -Msix dist/stitchcraft-windows-x86_64.msix
#>
param([Parameter(Mandatory)][string]$Msix)

. "$PSScriptRoot\common.ps1"

# Keep in step with Publisher in packaging/msix/AppxManifest.xml.
$subject = "CN=2F6A3C8E-9B41-4D7A-8E5C-1A7B90D4C3E2"
$cerPath = Join-Path (Split-Path $Msix) "stitchcraft-msix-test.cer"

$cert = New-SelfSignedCertificate -Type Custom -Subject $subject `
    -KeyUsage DigitalSignature -FriendlyName "Stitchcraft MSIX test" -CertStoreLocation Cert:\CurrentUser\My `
    -TextExtension @("2.5.29.37={text}1.3.6.1.5.5.7.3.3", "2.5.29.19={text}")
Export-Certificate -Cert $cert -FilePath $cerPath | Out-Null

$signtool = Find-SdkTool "signtool.exe"
& $signtool sign /fd SHA256 /sha1 $cert.Thumbprint $Msix
if ($LASTEXITCODE -ne 0) { throw "signtool failed" }
Write-Host "Signed $Msix, certificate at $cerPath"
