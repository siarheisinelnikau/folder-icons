param([switch]$Offline)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $projectRoot
$metadataText = & cargo metadata --locked --offline --no-deps --format-version 1
if ($LASTEXITCODE -ne 0) { throw 'Cargo metadata failed' }
$metadata = $metadataText | ConvertFrom-Json
$version = ($metadata.packages | Where-Object name -eq 'folder-icons').version
if ($version -notmatch '^\d+\.\d+\.\d+$') { throw 'Expected a stable package version' }
$cargoArguments = @('build', '--release', '--locked', '-j', '2', '--target', 'x86_64-pc-windows-msvc', '--target-dir', (Join-Path $projectRoot 'target'))
if ($Offline) { $cargoArguments += '--offline' }
& cargo @cargoArguments
if ($LASTEXITCODE -ne 0) { throw 'Release compilation failed' }
$distribution = Join-Path $projectRoot 'dist'
New-Item -ItemType Directory -Force -Path $distribution | Out-Null
$assetName = "FolderIcons-v$version.exe"
$destination = Join-Path $distribution $assetName
Copy-Item -LiteralPath (Join-Path $projectRoot 'target/x86_64-pc-windows-msvc/release/FolderIcons.exe') -Destination $destination -Force
$checksum = (Get-FileHash -LiteralPath $destination -Algorithm SHA256).Hash.ToLowerInvariant()
[System.IO.File]::WriteAllText("$destination.sha256", "$checksum  $assetName`n", [System.Text.UTF8Encoding]::new($false))
& (Join-Path $PSScriptRoot 'verify-package.ps1') -AssetPath $destination
& (Join-Path $PSScriptRoot 'third-party-notices.ps1') -OutputDirectory $distribution
Write-Output "Packaged $destination"
Write-Output "SHA256 $checksum"
