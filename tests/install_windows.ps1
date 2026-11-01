$ErrorActionPreference = 'Stop'
$root = Join-Path ([IO.Path]::GetTempPath()) ([guid]::NewGuid().ToString())
New-Item -ItemType Directory -Path $root | Out-Null
$originalDirectory = $env:TUNERIFT_INSTALL_DIR
$originalVersion = $env:TUNERIFT_VERSION
$originalNoPath = $env:TUNERIFT_NO_MODIFY_PATH
$tuneriftInstallerFixture = @{ BadChecksum = $false; Asset = $null; Archive = $null }
function Invoke-RestMethod { param($Uri) return @{ tag_name = 'v0.1.0' } }
# Keep mock state in the parent test scope. $script: would resolve to the
# installer script's scope when the mock is invoked from a separate .ps1 file.
function Invoke-WebRequest {
    param([switch]$UseBasicParsing, $Uri, $OutFile)
    if ($Uri.EndsWith('/SHA256SUMS')) {
        $hash = if ($tuneriftInstallerFixture.BadChecksum) { '0' * 64 } else { (Get-FileHash $tuneriftInstallerFixture.Archive -Algorithm SHA256).Hash.ToLowerInvariant() }
        "$hash  $($tuneriftInstallerFixture.Asset)" | Set-Content -LiteralPath $OutFile -Encoding ascii
    } elseif ($Uri.EndsWith('/' + $tuneriftInstallerFixture.Asset)) {
        Copy-Item -LiteralPath $tuneriftInstallerFixture.Archive -Destination $OutFile
    } else { throw "Unexpected URL: $Uri" }
}
try {
    $architecture = $env:PROCESSOR_ARCHITEW6432
    if (-not $architecture) { $architecture = $env:PROCESSOR_ARCHITECTURE }
    $target = if ($architecture -eq 'ARM64') { 'windows-arm64' } else { 'windows-x86_64' }
    $name = "tunerift-v0.1.0-$target"
    $package = Join-Path $root $name
    New-Item -ItemType Directory -Path $package | Out-Null
    [IO.File]::WriteAllText((Join-Path $package 'tunerift.exe'), 'fixture executable')
    $tuneriftInstallerFixture.Asset = "$name.zip"
    $tuneriftInstallerFixture.Archive = Join-Path $root $tuneriftInstallerFixture.Asset
    Compress-Archive -Path $package -DestinationPath $tuneriftInstallerFixture.Archive
    $env:TUNERIFT_INSTALL_DIR = Join-Path $root 'installation with spaces'
    $env:TUNERIFT_NO_MODIFY_PATH = '1'
    Remove-Item Env:TUNERIFT_VERSION -ErrorAction SilentlyContinue
    $installer = Join-Path $PSScriptRoot '..\install.ps1'
    & $installer
    $binary = Join-Path $env:TUNERIFT_INSTALL_DIR 'tunerift.exe'
    if ([IO.File]::ReadAllText($binary) -ne 'fixture executable') { throw 'Wrong installed content' }
    $env:TUNERIFT_VERSION = '0.1.0'
    & $installer
    # Exercise the documented `irm ... | iex` entry point as well as file invocation.
    Invoke-Expression (Get-Content -LiteralPath $installer -Raw)
    [IO.File]::WriteAllText($binary, 'previous executable')
    $tuneriftInstallerFixture.BadChecksum = $true
    $rejected = $false
    try { & $installer } catch { $rejected = $true }
    if (-not $rejected) { throw 'A bad checksum was accepted' }
    if ([IO.File]::ReadAllText($binary) -ne 'previous executable') { throw 'Existing executable was changed on failure' }
    $tuneriftInstallerFixture.BadChecksum = $false
    $env:TUNERIFT_VERSION = '../../bad'
    $rejected = $false
    try { & $installer } catch { $rejected = $true }
    if (-not $rejected) { throw 'An invalid version was accepted' }
    Write-Host 'PASS: Windows install/update, latest/pinned version, spaces, checksum failure and invalid version'
} finally {
    $env:TUNERIFT_INSTALL_DIR = $originalDirectory
    $env:TUNERIFT_VERSION = $originalVersion
    $env:TUNERIFT_NO_MODIFY_PATH = $originalNoPath
    Remove-Item -LiteralPath $root -Recurse -Force
}
