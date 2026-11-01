# Install an official Windows release into the current user's directory.
& {
    $ErrorActionPreference = 'Stop'
    [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
    if ($env:OS -ne 'Windows_NT') { throw 'Use install.sh on Linux or macOS.' }
    $architecture = $env:PROCESSOR_ARCHITEW6432
    if (-not $architecture) { $architecture = $env:PROCESSOR_ARCHITECTURE }
    switch ($architecture.ToUpperInvariant()) {
        'AMD64' { $target = 'windows-x86_64' }
        'ARM64' { $target = 'windows-arm64' }
        default { throw 'Supported CPU architectures: x86_64 and ARM64.' }
    }
    $repository = 'https://github.com/Nyakooo/tunerift'
    $version = $env:TUNERIFT_VERSION
    if (-not $version) {
        $release = Invoke-RestMethod -Uri 'https://api.github.com/repos/Nyakooo/tunerift/releases/latest'
        $version = $release.tag_name
        if (-not $version) { throw 'No published release is available yet.' }
    }
    $tag = if ($version.StartsWith('v')) { $version } else { "v$version" }
    if ($tag -notmatch '^v[0-9][a-zA-Z0-9.+-]*$') { throw "Invalid version: $tag" }
    $name = "tunerift-$tag-$target"
    $asset = "$name.zip"
    $directory = $env:TUNERIFT_INSTALL_DIR
    if (-not $directory) { $directory = Join-Path $env:LOCALAPPDATA 'Tunerift\bin' }
    $directory = [IO.Path]::GetFullPath($directory)
    $temporary = Join-Path ([IO.Path]::GetTempPath()) ([guid]::NewGuid().ToString())
    $staged = $null
    New-Item -ItemType Directory -Path $temporary | Out-Null
    try {
        Write-Host "Downloading Tunerift $tag for $target..."
        $archive = Join-Path $temporary $asset
        $checksums = Join-Path $temporary 'SHA256SUMS'
        Invoke-WebRequest -UseBasicParsing -Uri "$repository/releases/download/$tag/$asset" -OutFile $archive
        Invoke-WebRequest -UseBasicParsing -Uri "$repository/releases/download/$tag/SHA256SUMS" -OutFile $checksums
        $entries = @(Get-Content -LiteralPath $checksums | Where-Object { $_ -match ('^[a-fA-F0-9]{64}\s+' + [regex]::Escape($asset) + '$') })
        if ($entries.Count -ne 1) { throw 'Missing or invalid package checksum; nothing was installed.' }
        $expected = ($entries[0] -split '\s+')[0]
        if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -ne $expected) {
            throw 'SHA256 mismatch; nothing was installed.'
        }
        Expand-Archive -LiteralPath $archive -DestinationPath $temporary
        $binary = Join-Path $temporary "$name\tunerift.exe"
        if (-not (Test-Path -LiteralPath $binary -PathType Leaf)) { throw 'Archive does not contain tunerift.exe.' }
        New-Item -ItemType Directory -Path $directory -Force | Out-Null
        $destination = Join-Path $directory 'tunerift.exe'
        $staged = Join-Path $directory ('.tunerift-' + [guid]::NewGuid().ToString() + '.exe')
        Copy-Item -LiteralPath $binary -Destination $staged
        if (Test-Path -LiteralPath $destination) {
            [IO.File]::Replace($staged, $destination, $null)
        } else {
            [IO.File]::Move($staged, $destination)
        }
        $staged = $null
        if ($env:TUNERIFT_NO_MODIFY_PATH -ne '1') {
            $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
            $entries = @($userPath -split ';' | Where-Object { $_ })
            if ($entries -notcontains $directory) {
                [Environment]::SetEnvironmentVariable('Path', (($entries + $directory) -join ';'), 'User')
            }
            if (($env:Path -split ';') -notcontains $directory) { $env:Path = "$directory;$env:Path" }
        }
        Write-Host "Installed: $destination"
        Write-Host 'Run: tunerift (open a new terminal if needed).'
    } finally {
        if ($staged -and (Test-Path -LiteralPath $staged)) { Remove-Item -LiteralPath $staged -Force }
        Remove-Item -LiteralPath $temporary -Recurse -Force
    }
}
