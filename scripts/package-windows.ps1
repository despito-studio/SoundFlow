param([string]$Destination, [switch]$SkipInstaller, [switch]$SkipZip)
$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$versionMatch = [regex]::Match((Get-Content -LiteralPath (Join-Path $projectRoot 'Cargo.toml') -Raw), '(?m)^version\s*=\s*"([^"]+)"\s*$')
if (-not $versionMatch.Success) { throw 'The package version is missing from Cargo.toml.' }
$version = $versionMatch.Groups[1].Value
$releaseRoot = Join-Path $projectRoot 'output/flutter-preview'
$releaseExe = Join-Path $releaseRoot 'SoundFlow.exe'
if (-not $Destination) { $Destination = Join-Path $projectRoot 'dist/windows-x64' }
$destinationPath = [IO.Path]::GetFullPath($Destination)
$distributionRoot = Join-Path $projectRoot 'dist'
if (-not $destinationPath.StartsWith($distributionRoot + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) {
    throw 'The package destination must be a subdirectory of this project dist folder.'
}
foreach ($required in @($releaseExe, (Join-Path $projectRoot 'resources/runtime/soundflow-node.exe'), (Join-Path $projectRoot 'media-provider/dist/worker.cjs'))) {
    if (-not (Test-Path -LiteralPath $required -PathType Leaf)) { throw "Missing native release dependency: $required" }
}
$releaseIdentity = (Get-Item -LiteralPath $releaseExe).VersionInfo
if ($releaseIdentity.ProductName -ne 'SoundFlow' -or $releaseIdentity.FileDescription -ne 'SoundFlow Music' -or
    $releaseIdentity.ProductVersion -notmatch ('^' + [regex]::Escape($version) + '(?:\.0|\+\d+)?$')) {
    throw 'The release executable is not the current Flutter SoundFlow build. Run scripts/build-flutter-windows.ps1 first.'
}
foreach ($path in @($distributionRoot,$destinationPath)) {
    if ((Test-Path -LiteralPath $path) -and ((Get-Item -LiteralPath $path).Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Package paths must not be junctions.' }
}
if (Test-Path -LiteralPath $destinationPath) { Remove-Item -LiteralPath $destinationPath -Recurse -Force }
New-Item -ItemType Directory -Path $destinationPath -Force | Out-Null
Get-ChildItem -LiteralPath $releaseRoot | Where-Object Name -notin @('startup-stderr.txt','resources','media-provider','licenses') | Copy-Item -Destination $destinationPath -Recurse -Force
$resourceFolder = Join-Path $destinationPath 'resources'
New-Item -ItemType Directory -Path $resourceFolder -Force | Out-Null
foreach ($folder in @('runtime','covers','licenses')) {
    Copy-Item -LiteralPath (Join-Path $projectRoot "resources/$folder") -Destination $resourceFolder -Recurse
}
foreach ($file in @('icon.png','icon.ico','icon-rose.ico','icon-violet.ico','icon-ocean.ico','curated.json')) {
    Copy-Item -LiteralPath (Join-Path $projectRoot "resources/$file") -Destination $resourceFolder
}
$workerFolder = Join-Path $destinationPath 'media-provider/dist'
New-Item -ItemType Directory -Path $workerFolder -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $projectRoot 'media-provider/dist/worker.cjs') -Destination $workerFolder
$licenses = Join-Path $destinationPath 'licenses'
New-Item -ItemType Directory -Path $licenses -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $releaseRoot 'licenses/FLUTTER-LICENSE.txt') -Destination $licenses
Copy-Item -LiteralPath (Join-Path $projectRoot 'resources/runtime/NODE-LICENSE.txt') -Destination $licenses
Copy-Item -LiteralPath (Join-Path $projectRoot 'resources/ARTWORK.md') -Destination $licenses
Get-ChildItem -LiteralPath (Join-Path $projectRoot 'resources/licenses') -File | Copy-Item -Destination $licenses
Copy-Item -LiteralPath (Join-Path $projectRoot 'resources/icons/flutter-sources.json') -Destination (Join-Path $licenses 'ANIMATEICONS-SOURCES.json')
Copy-Item -LiteralPath (Join-Path $projectRoot 'media-provider/dist/THIRD-PARTY-LICENSES.txt') -Destination (Join-Path $licenses 'MEDIA-LICENSES.txt')
& (Join-Path $projectRoot 'scripts/collect-licenses.ps1') -OutputPath (Join-Path $licenses 'RUST-LICENSES.txt')
if (Test-Path -LiteralPath (Join-Path $projectRoot 'media-provider/licenses')) {
    Copy-Item -LiteralPath (Join-Path $projectRoot 'media-provider/licenses') -Destination (Join-Path $licenses 'media') -Recurse
}
$notes = @'
SoundFlow — native Windows x64

Run SoundFlow.exe. Keep its DLLs, data, resources and media-provider beside it.
No Python, Node.js installation, browser, or WebView2 is required.
The interface runs in Flutter and audio in Rust. Online music starts at most
one bundled JavaScript media worker, which exits with SoundFlow.

Library and settings: %APPDATA%\app.soundflow.desktop
Installation and uninstallation leave your music and library in place.
Cloud accounts, synchronization and Plus need a configured HTTPS service.
This package does not deploy an account or payment server.
'@
Set-Content -LiteralPath (Join-Path $destinationPath 'READ-ME.txt') -Value $notes -Encoding utf8
if (-not $SkipZip) {
    $zipPath = Join-Path $distributionRoot "SoundFlow-$version-windows-x64.zip"
    Compress-Archive -Path (Join-Path $destinationPath '*') -DestinationPath $zipPath -Force
}
if (-not $SkipInstaller) {
    $nsis = Get-Command makensis.exe -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source -First 1
    if (-not $nsis) { $nsis = Join-Path $projectRoot 'output/toolchain/nsis/makensis.exe' }
    if (-not (Test-Path -LiteralPath $nsis)) { throw 'Install NSIS to build the native installer, or use -SkipInstaller for a portable package.' }
    $installerPath = Join-Path $distributionRoot "SoundFlow-$version-Setup.exe"
    & $nsis '/WX' '/V2' "/DSTAGE=$destinationPath" "/DPROJECT=$projectRoot" "/DOUTPUT=$installerPath" "/DVERSION=$version" (Join-Path $projectRoot 'resources/installer/windows.nsi')
    if ($LASTEXITCODE -ne 0) { throw 'Native NSIS installer creation failed.' }
}
Get-ChildItem -LiteralPath $distributionRoot -File | Select-Object Name, Length
