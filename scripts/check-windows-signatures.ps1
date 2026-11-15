# Checks that a signed Windows build is signed where it matters: the application, the NSIS
# installer and the MSI, each with a valid signature and a timestamp.
#
#   ./scripts/check-windows-signatures.ps1 target/x86_64-pc-windows-msvc/release
#
# "Valid" is Windows' own verdict, so the certificate's chain has to be trusted on the machine
# running this: true of a certificate from a public authority, and of a test certificate only
# once it is added to the trusted roots.
param([Parameter(Mandatory)][string]$Release)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$files = @(
  Get-Item (Join-Path $Release 'prune-desktop.exe')
  Get-ChildItem (Join-Path $Release 'bundle\nsis\*-setup.exe')
  Get-ChildItem (Join-Path $Release 'bundle\msi\*.msi')
)
if ($files.Count -lt 3) { throw "Expected the application, an NSIS installer and an MSI under $Release; found $($files.Count) file(s)." }

$problems = 0
foreach ($f in $files) {
  $s = Get-AuthenticodeSignature $f.FullName
  $signer = if ($s.SignerCertificate) { $s.SignerCertificate.Subject } else { 'no signer' }
  Write-Host ('{0,-14} {1}  {2}' -f $s.Status, $f.Name, $signer)
  if ($s.Status -ne 'Valid') { Write-Host "  $($s.StatusMessage)"; $problems++ }
  elseif (-not $s.TimeStamperCertificate) { Write-Host '  not timestamped: invalid once the certificate expires'; $problems++ }
}
if ($problems) { throw "$problems of $($files.Count) files are not properly signed." }
Write-Host "All $($files.Count) files are signed and timestamped."
