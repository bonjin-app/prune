# Makes the next `tauri build` on Windows sign what it produces: the application, the NSIS
# installer and its uninstaller, and the MSI.
#
# Reads the certificate from the environment, the way the release workflow hands over secrets:
#
#   WINDOWS_CERTIFICATE           base64 of a code-signing .pfx that includes its private key
#   WINDOWS_CERTIFICATE_PASSWORD  that .pfx's password (may be empty)
#   WINDOWS_TIMESTAMP_URL         optional; an RFC 3161 timestamp server
#
# It imports the certificate into the current user's store and writes its thumbprint to
# src-tauri/tauri.windows.conf.json, which Tauri merges into its configuration on Windows builds
# only. That file is generated, holds no secret, and is ignored by git.
#
# Unsigned, the NSIS installer cannot be installed by winget with nobody at the keyboard: see
# issue #1. Signing is what lets it back into the WinGet manifest.
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

if ([string]::IsNullOrEmpty($env:WINDOWS_CERTIFICATE)) {
  throw 'WINDOWS_CERTIFICATE is empty: there is no certificate to sign with.'
}

$pfx = Join-Path ([IO.Path]::GetTempPath()) "prune-signing-$PID.pfx"
try {
  [IO.File]::WriteAllBytes($pfx, [Convert]::FromBase64String($env:WINDOWS_CERTIFICATE))
  $import = @{ FilePath = $pfx; CertStoreLocation = 'Cert:\CurrentUser\My' }
  if (-not [string]::IsNullOrEmpty($env:WINDOWS_CERTIFICATE_PASSWORD)) {
    $import.Password = ConvertTo-SecureString $env:WINDOWS_CERTIFICATE_PASSWORD -AsPlainText -Force
  }
  # A .pfx can carry its issuing chain as well; the one to sign with is the one with a key.
  $cert = Import-PfxCertificate @import | Where-Object HasPrivateKey | Select-Object -First 1
} finally {
  Remove-Item $pfx -Force -ErrorAction SilentlyContinue
}

if (-not $cert) { throw 'The certificate has no private key, so nothing can be signed with it.' }
$codeSigning = '1.3.6.1.5.5.7.3.3'
if (-not ($cert.EnhancedKeyUsageList.ObjectId -contains $codeSigning)) {
  throw "The certificate for $($cert.Subject) is not for code signing."
}
if ($cert.NotAfter -lt (Get-Date)) { throw "The certificate for $($cert.Subject) expired on $($cert.NotAfter)." }

$timestamp = if ($env:WINDOWS_TIMESTAMP_URL) { $env:WINDOWS_TIMESTAMP_URL } else { 'http://timestamp.digicert.com' }
$config = [ordered]@{
  bundle = [ordered]@{
    windows = [ordered]@{
      certificateThumbprint = $cert.Thumbprint
      digestAlgorithm       = 'sha256'
      # Without a timestamp a signature stops being valid the day the certificate expires.
      timestampUrl          = $timestamp
      tsp                   = $true
    }
  }
}
$path = Join-Path $PSScriptRoot '..\src-tauri\tauri.windows.conf.json'
$config | ConvertTo-Json -Depth 4 | Set-Content -Path $path -Encoding utf8

Write-Host "Windows builds will be signed by $($cert.Subject) ($($cert.Thumbprint)), valid until $($cert.NotAfter.ToString('yyyy-MM-dd')), timestamped by $timestamp."
