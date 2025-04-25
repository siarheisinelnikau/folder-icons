param([Parameter(Mandatory=$true)][string]$OutputDirectory)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $projectRoot
$metadataText = & cargo metadata --locked --offline --format-version 1 --filter-platform x86_64-pc-windows-msvc
if ($LASTEXITCODE -ne 0) { throw 'Cannot resolve dependency license metadata' }
$metadata = $metadataText | ConvertFrom-Json
$noticeDirectory = Join-Path $OutputDirectory 'licenses'
New-Item -ItemType Directory -Force -Path $noticeDirectory | Out-Null
Copy-Item -LiteralPath (Join-Path $projectRoot 'LICENSE') -Destination (Join-Path $noticeDirectory 'FolderIcons-MIT.txt') -Force
$lines = [System.Collections.Generic.List[string]]::new()
$lines.Add('FolderIcons third-party dependency licenses')
$lines.Add('Generated from Cargo.lock and locally resolved crate sources. Includes transitive and build dependencies.')
foreach ($package in ($metadata.packages | Where-Object source | Sort-Object name,version)) {
    $lines.Add("$($package.name) $($package.version) | $($package.license) | $($package.repository)")
    $crateRoot = Split-Path -Parent $package.manifest_path
    $files = Get-ChildItem -LiteralPath $crateRoot -File -Recurse | Where-Object {
        ($_.Name -match '(?i)(license|licence|copying|notice|OFL|UFL)' -and $_.Extension -ne '.rs') -or
        ($_.Extension -eq '.txt' -and $_.FullName -match '[\\/]fonts[\\/]')
    }
    foreach ($file in $files) {
        $relativeName = $file.FullName.Substring($crateRoot.Length + 1).Replace('\', '_').Replace('/', '_')
        $name = "$($package.name)-$($package.version)-$relativeName"
        Copy-Item -LiteralPath $file.FullName -Destination (Join-Path $noticeDirectory $name) -Force
    }
}
[System.IO.File]::WriteAllLines((Join-Path $OutputDirectory 'THIRD-PARTY-NOTICES.txt'), $lines, [System.Text.UTF8Encoding]::new($false))
