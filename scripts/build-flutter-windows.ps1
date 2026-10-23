param([switch]$SkipCore)
$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$appRoot = Join-Path $projectRoot 'flutter_app'
$flutter = Join-Path $projectRoot 'output/toolchain/flutter/bin/flutter.bat'
$cmake = Join-Path $projectRoot 'output/toolchain/python-build/cmake/data/bin/cmake.exe'
$ninja = Join-Path $projectRoot 'output/toolchain/python-build/bin/ninja.exe'
if (-not (Test-Path -LiteralPath $flutter)) { $flutter = (Get-Command flutter.bat -ErrorAction Stop).Source }
if (-not (Test-Path -LiteralPath $cmake)) { $cmake = (Get-Command cmake.exe -ErrorAction Stop).Source }
if (-not (Test-Path -LiteralPath $ninja)) { $ninja = (Get-Command ninja.exe -ErrorAction Stop).Source }
$buildDir = Join-Path $appRoot 'build/windows-ninja'
$bundle = [IO.Path]::GetFullPath((Join-Path $projectRoot 'output/flutter-preview'))
if (-not $bundle.StartsWith($projectRoot + '\', [StringComparison]::OrdinalIgnoreCase)) { throw 'Bundle must stay within the checkout.' }
if ((Test-Path -LiteralPath $bundle) -and ((Get-Item -LiteralPath $bundle).Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Bundle must not be a junction.' }
if (-not (Test-Path -LiteralPath $flutter)) { throw 'Flutter SDK is missing from output/toolchain/flutter.' }
& (Join-Path $PSScriptRoot 'build-windows.ps1') -Action Environment
if (-not $SkipCore) {
    & (Join-Path $PSScriptRoot 'build-windows.ps1') -Action Build
    if ($LASTEXITCODE -ne 0) { throw 'Audio core compilation failed.' }
}
$env:FLUTTER_SUPPRESS_ANALYTICS = 'true'
$env:CI = 'true'
Push-Location $appRoot
try {
    & $flutter pub get
    if ($LASTEXITCODE -ne 0) { throw 'Flutter dependencies could not be resolved.' }
    & $flutter build windows --release --config-only
    $config = Join-Path $appRoot 'windows/flutter/ephemeral/generated_config.cmake'
    if (-not (Test-Path -LiteralPath $config)) { throw 'Flutter did not generate its Windows configuration.' }
    $rcForCmake = $env:RC.Replace('\','/')
    & $cmake -S windows -B $buildDir -G Ninja '-DCMAKE_BUILD_TYPE=Release' "-DCMAKE_MAKE_PROGRAM=$ninja" '-DCMAKE_CXX_COMPILER=cl.exe' "-DCMAKE_RC_COMPILER=$rcForCmake" "-DCMAKE_INSTALL_PREFIX=$bundle"
    if ($LASTEXITCODE -ne 0) { throw 'Windows host configuration failed.' }
    if (-not (Test-Path -LiteralPath (Join-Path $appRoot 'windows/flutter/ephemeral/flutter_windows.dll.lib'))) {
        & $cmake --build $buildDir --target flutter_assemble --parallel 2
        if ($LASTEXITCODE -ne 0) { throw 'Flutter rendering assets could not be compiled.' }
    }
    & $cmake --build $buildDir --parallel 2
    if ($LASTEXITCODE -ne 0) { throw 'Flutter Windows compilation failed.' }
    & $cmake --install $buildDir --config Release
    if ($LASTEXITCODE -ne 0) { throw 'Flutter Windows bundle could not be assembled.' }
    Copy-Item -LiteralPath (Join-Path $projectRoot 'target/release/soundflow_lib.dll') -Destination $bundle -Force
    New-Item -ItemType Directory -Path (Join-Path $bundle 'licenses') -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path (Split-Path (Split-Path $flutter)) 'LICENSE') -Destination (Join-Path $bundle 'licenses/FLUTTER-LICENSE.txt') -Force
    New-Item -ItemType Directory -Path (Join-Path $bundle 'resources') -Force | Out-Null
    foreach ($name in @('covers','runtime','licenses')) {
        $source = Join-Path $projectRoot "resources/$name"
        if (Test-Path -LiteralPath $source) { Copy-Item -LiteralPath $source -Destination (Join-Path $bundle 'resources') -Recurse -Force }
    }
    foreach ($name in @('icon.png','icon.ico','icon-rose.ico','icon-violet.ico','icon-ocean.ico','curated.json')) {
        Copy-Item -LiteralPath (Join-Path $projectRoot "resources/$name") -Destination (Join-Path $bundle 'resources') -Force
    }
    $provider = Join-Path $projectRoot 'media-provider/dist'
    if (Test-Path -LiteralPath $provider) {
        New-Item -ItemType Directory -Path (Join-Path $bundle 'media-provider') -Force | Out-Null
        Copy-Item -LiteralPath $provider -Destination (Join-Path $bundle 'media-provider') -Recurse -Force
    }
    Write-Output "SoundFlow Flutter: $bundle\SoundFlow.exe"
} finally { Pop-Location }
