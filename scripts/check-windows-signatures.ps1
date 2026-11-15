# Checks that a signed Windows build is signed where it matters: the NSIS installer, the MSI, and
# the application inside each of them, every one with a valid signature and a timestamp.
#
#   ./scripts/check-windows-signatures.ps1 target/x86_64-pc-windows-msvc/release
#
# The application is checked as unpacked from each installer, not as left in the build folder.
# Tauri patches that copy for each installer type and signs it before packing it, so the file
# left behind afterwards need not be the signed one, and is not what anyone installs.
#
# Unpacking the NSIS installer needs 7-Zip (`7z`), which GitHub's Windows runners have.
#
# "Valid" is Windows' own verdict, so the certificate's chain has to be trusted on the machine
# running this: true of a certificate from a public authority, and of a test certificate only
# once it is added to the trusted roots.
param([Parameter(Mandatory)][string]$Release)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$nsis = @(Get-ChildItem (Join-Path $Release 'bundle\nsis\*-setup.exe'))
$msi = @(Get-ChildItem (Join-Path $Release 'bundle\msi\*.msi'))
if ($nsis.Count -ne 1 -or $msi.Count -ne 1) {
  throw "Expected one NSIS installer and one MSI under $Release; found $($nsis.Count) and $($msi.Count)."
}

$work = Join-Path ([IO.Path]::GetTempPath()) "prune-signatures-$PID"
New-Item -ItemType Directory -Path $work -Force | Out-Null
try {
  $fromMsi = Join-Path $work 'msi'
  $p = Start-Process msiexec.exe -ArgumentList "/a `"$($msi[0].FullName)`" /qn TARGETDIR=`"$fromMsi`"" -Wait -PassThru
  if ($p.ExitCode -ne 0) { throw "Unpacking the MSI failed with $($p.ExitCode)." }

  if (-not (Get-Command 7z -ErrorAction SilentlyContinue)) { throw '7z is needed to unpack the NSIS installer.' }
  $fromNsis = Join-Path $work 'nsis'
  & 7z x $nsis[0].FullName "-o$fromNsis" -y | Out-Null
  if ($LASTEXITCODE -ne 0) { throw "Unpacking the NSIS installer failed with $LASTEXITCODE." }

  $checks = [ordered]@{
    $nsis[0].Name                     = $nsis[0].FullName
    $msi[0].Name                      = $msi[0].FullName
    'prune-desktop.exe from the NSIS' = $fromNsis
    'prune-desktop.exe from the MSI'  = $fromMsi
  }
  $problems = 0
  foreach ($name in $checks.Keys) {
    $path = $checks[$name]
    if (Test-Path $path -PathType Container) {
      $found = @(Get-ChildItem $path -Recurse -Filter prune-desktop.exe)
      if ($found.Count -ne 1) { Write-Host "MISSING        $name"; $problems++; continue }
      $path = $found[0].FullName
    }
    $s = Get-AuthenticodeSignature $path
    $signer = if ($s.SignerCertificate) { $s.SignerCertificate.Subject } else { 'no signer' }
    Write-Host ('{0,-14} {1}  {2}' -f $s.Status, $name, $signer)
    if ($s.Status -ne 'Valid') { Write-Host "  $($s.StatusMessage)"; $problems++ }
    elseif (-not $s.TimeStamperCertificate) { Write-Host '  not timestamped: invalid once the certificate expires'; $problems++ }
  }
} finally {
  Remove-Item $work -Recurse -Force -ErrorAction SilentlyContinue
}
if ($problems) { throw "$problems of $($checks.Count) checks failed." }
Write-Host "All $($checks.Count) are signed and timestamped."
