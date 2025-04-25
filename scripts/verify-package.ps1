param([Parameter(Mandatory=$true)][string]$AssetPath)
$ErrorActionPreference = 'Stop'
# Inspect bytes only. Never execute a release binary as part of packaging.
$bytes = [System.IO.File]::ReadAllBytes($AssetPath)
if ($bytes.Length -lt 96 -or $bytes[0] -ne 0x4d -or $bytes[1] -ne 0x5a) { throw 'Missing DOS executable header' }
$peOffset = [BitConverter]::ToInt32($bytes, 60)
if ($peOffset -lt 64 -or $peOffset -gt $bytes.Length - 94) { throw 'Invalid PE offset' }
if ([BitConverter]::ToUInt32($bytes, $peOffset) -ne 0x4550) { throw 'Missing PE signature' }
if ([BitConverter]::ToUInt16($bytes, $peOffset + 4) -ne 0x8664) { throw 'Expected x64 architecture' }
if ([BitConverter]::ToUInt16($bytes, $peOffset + 24) -ne 0x20b) { throw 'Expected PE32+ optional header' }
if ([BitConverter]::ToUInt16($bytes, $peOffset + 24 + 68) -ne 2) { throw 'Expected Windows GUI subsystem' }
$checksumParts = (Get-Content -LiteralPath "$AssetPath.sha256" -Raw).Trim() -split '\s+'
if ($checksumParts.Length -ne 2 -or $checksumParts[1] -ne [System.IO.Path]::GetFileName($AssetPath)) { throw 'Checksum asset name mismatch' }
$actualHash = (Get-FileHash -LiteralPath $AssetPath -Algorithm SHA256).Hash.ToLowerInvariant()
if ($checksumParts[0] -ne $actualHash) { throw 'Package SHA-256 mismatch' }
Write-Output "Verified x64 PE32+ GUI executable and SHA-256 ($($bytes.Length) bytes)."
