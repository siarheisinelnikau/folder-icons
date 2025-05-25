param([string]$FixtureRoot)
$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
Set-Location -LiteralPath $projectRoot
if (-not $FixtureRoot) { $FixtureRoot = Join-Path $projectRoot '.test-output/visual-fixtures' }
$FixtureRoot = [System.IO.Path]::GetFullPath($FixtureRoot)
$marker = Join-Path $FixtureRoot '.foldericons-visual-fixture'
if (Test-Path -LiteralPath $FixtureRoot) {
    if (-not (Test-Path -LiteralPath $marker) -or (Get-Content -LiteralPath $marker -Raw) -ne 'FolderIcons disposable fixtures v1') {
        throw 'Refusing to claim an existing fixture directory without the expected marker'
    }
    Write-Output $FixtureRoot
    return
}
New-Item -ItemType Directory -Path (Join-Path $FixtureRoot 'Project Atlas'), (Join-Path $FixtureRoot 'Photos 旅行') | Out-Null
[System.IO.File]::WriteAllText($marker, 'FolderIcons disposable fixtures v1')
$original = "; Disposable visual fixture`r`n[.ShellClassInfo]`r`nInfoTip=資料`r`nIconResource=%SystemRoot%\system32\shell32.dll,3`r`n[ViewState]`r`nMode=3`r`n"
[System.IO.File]::WriteAllText((Join-Path $FixtureRoot 'Project Atlas/desktop.ini'), $original, [System.Text.UnicodeEncoding]::new($false, $true))
Copy-Item -LiteralPath (Join-Path $projectRoot 'tests/fixtures/cover.png') -Destination (Join-Path $FixtureRoot 'cover.png')
Write-Output $FixtureRoot
