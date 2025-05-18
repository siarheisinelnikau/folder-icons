param(
    [Parameter(Mandatory=$true)][string]$FixtureRoot,
    [string]$OutputDirectory,
    [switch]$Diagnostics
)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $projectRoot
$exePath = Join-Path $projectRoot 'target/x86_64-pc-windows-msvc/release/FolderIcons.exe'
$FixtureRoot = (Resolve-Path -LiteralPath $FixtureRoot).Path
if (-not $OutputDirectory) { $OutputDirectory = Join-Path $projectRoot '.local/visual-review' }
$OutputDirectory = [System.IO.Path]::GetFullPath($OutputDirectory)
New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
function Invoke-OwnedDiagnostic([string[]]$AppArguments, [int]$TimeoutMilliseconds) {
    $quotedArguments = $AppArguments | ForEach-Object { '"' + $_.Replace('"', '') + '"' }
    $captureProcess = Start-Process -FilePath $exePath -ArgumentList $quotedArguments -WorkingDirectory $projectRoot -WindowStyle Hidden -PassThru
    if (-not $captureProcess.WaitForExit($TimeoutMilliseconds)) {
        Stop-Process -Id $captureProcess.Id -ErrorAction SilentlyContinue
        throw "Owned capture/diagnostic exceeded its $TimeoutMilliseconds ms limit"
    }
    if ($captureProcess.ExitCode -ne 0) { throw "Capture/diagnostic failed: exit $($captureProcess.ExitCode). Inspect the EXE's error.log." }
}
if ($Diagnostics) {
    $performancePath = Join-Path $OutputDirectory 'performance.json'
    Invoke-OwnedDiagnostic @('--diagnose', $performancePath, $FixtureRoot) 35000
    $performance = Get-Content -LiteralPath $performancePath -Raw | ConvertFrom-Json
    if (-not $performance.gate_passed) { throw "Rendering gate did not pass. Inspect $performancePath before further runtime testing." }
}
Invoke-OwnedDiagnostic @('--capture', (Join-Path $OutputDirectory 'main.png'), '--screen', 'main', '--scenario', 'review', '--fixture-root', $FixtureRoot) 12000
Invoke-OwnedDiagnostic @('--capture', (Join-Path $OutputDirectory 'settings.png'), '--screen', 'settings', '--scenario', 'populated', '--fixture-root', $FixtureRoot) 12000
Invoke-OwnedDiagnostic @('--capture', (Join-Path $OutputDirectory 'compact.png'), '--scenario', 'empty', '--width', '820', '--height', '620', '--scale', '1.5') 12000
Write-Output "Captured production client UI to $OutputDirectory. Every child process exited."
