param([string]$Destination, [switch]$SkipInstaller, [switch]$SkipZip)
$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$version = '3.5.1'
$releaseExe = Join-Path $projectRoot 'target/release/soundflow.exe'
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
    ($releaseIdentity.ProductVersion -notmatch '^3\.4\.' -and $releaseIdentity.ProductVersion -notmatch ('^' + [regex]::Escape($version) + '(?:\.0)?$'))) {
    throw 'The release executable is not the current native SoundFlow build. Run scripts/build-windows.ps1 -Action Build first.'
}
if (Test-Path -LiteralPath $destinationPath) { Remove-Item -LiteralPath $destinationPath -Recurse -Force }
New-Item -ItemType Directory -Path $destinationPath -Force | Out-Null
Copy-Item -LiteralPath $releaseExe -Destination (Join-Path $destinationPath 'SoundFlow.exe')
$resourceFolder = Join-Path $destinationPath 'resources'
New-Item -ItemType Directory -Path $resourceFolder -Force | Out-Null
foreach ($folder in @('runtime','covers')) {
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
Copy-Item -LiteralPath (Join-Path $projectRoot 'resources/runtime/NODE-LICENSE.txt') -Destination $licenses
Copy-Item -LiteralPath (Join-Path $projectRoot 'resources/ARTWORK.md') -Destination $licenses
Copy-Item -LiteralPath (Join-Path $projectRoot 'resources/licenses/SLINT-LICENSE.md') -Destination $licenses
Copy-Item -LiteralPath (Join-Path $projectRoot 'media-provider/dist/THIRD-PARTY-LICENSES.txt') -Destination (Join-Path $licenses 'MEDIA-LICENSES.txt')
& (Join-Path $projectRoot 'scripts/collect-licenses.ps1') -OutputPath (Join-Path $licenses 'RUST-LICENSES.txt')
if (Test-Path -LiteralPath (Join-Path $projectRoot 'media-provider/licenses')) {
    Copy-Item -LiteralPath (Join-Path $projectRoot 'media-provider/licenses') -Destination (Join-Path $licenses 'media') -Recurse
}
$notes = @'
SoundFlow 3.5.0 — native Windows x64

Run SoundFlow.exe. Keep resources and media-provider beside it.
No Python, Node.js installation, browser, or WebView2 is required.
The interface and audio output run in Rust. Online music starts at most
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
    & $nsis '/WX' '/V2' "/DSTAGE=$destinationPath" "/DPROJECT=$projectRoot" "/DOUTPUT=$installerPath" (Join-Path $projectRoot 'resources/installer/windows.nsi')
    if ($LASTEXITCODE -ne 0) { throw 'Native NSIS installer creation failed.' }
}
Get-ChildItem -LiteralPath $distributionRoot -File | Select-Object Name, Length
