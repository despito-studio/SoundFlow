param(
    [ValidateSet('Build', 'Check', 'Environment')]
    [string]$Action = 'Build'
)

$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
$vsRoot = $null
$msvcRoot = $null
if (Test-Path -LiteralPath $vswhere) {
    $vsRoot = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if ($vsRoot) {
        $msvcVersion = (Get-Content -LiteralPath (Join-Path $vsRoot 'VC/Auxiliary/Build/Microsoft.VCToolsVersion.default.txt') -Raw).Trim()
        $msvcRoot = Join-Path $vsRoot "VC/Tools/MSVC/$msvcVersion"
    }
}
if (-not $msvcRoot) {
    $portableVcRoot = Join-Path $projectRoot 'output/toolchain/msvc/VC'
    $portableToolset = Get-ChildItem -LiteralPath (Join-Path $portableVcRoot 'Tools/MSVC') -Directory -ErrorAction SilentlyContinue |
        Sort-Object Name -Descending | Select-Object -First 1
    if (-not $portableToolset) { throw 'Install Visual Studio C++ tools or run scripts/setup-msvc.ps1. See docs/WINDOWS.md.' }
    $msvcRoot = $portableToolset.FullName
    $msvcVersion = $portableToolset.Name
}
$msvcBin = Join-Path $msvcRoot 'bin/Hostx64/x64'
$cargoCandidates = @()
if ($env:CARGO_HOME) { $cargoCandidates += Join-Path $env:CARGO_HOME 'bin/cargo.exe' }
$cargoCommand = Get-Command cargo.exe -ErrorAction SilentlyContinue | Select-Object -First 1
if ($cargoCommand) { $cargoCandidates += $cargoCommand.Source }
$cargoCandidates += Join-Path $env:USERPROFILE '.cargo/bin/cargo.exe'
$cargoExe = $cargoCandidates | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } | Select-Object -First 1
if (-not $cargoExe) { throw 'Install Rust MSVC 1.92 or newer. The script searches CARGO_HOME, PATH and the current user profile.' }
$cargoBin = Split-Path $cargoExe

# Prefer an installed SDK. The local NuGet SDK is only a checkout-scoped fallback.
$sdkSystemRoot = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits/10'
$sdkIncludeVersions = Get-ChildItem -LiteralPath (Join-Path $sdkSystemRoot 'Include') -Directory -ErrorAction SilentlyContinue | Sort-Object Name -Descending
$sdkRoot = $sdkSystemRoot
$sdkVersion = $sdkIncludeVersions | Select-Object -First 1 -ExpandProperty Name
if ($sdkVersion) {
    $sdkBin = Join-Path $sdkRoot "bin/$sdkVersion/x64"
    $sdkUm = Join-Path $sdkRoot "Lib/$sdkVersion/um/x64"
    $sdkUcrt = Join-Path $sdkRoot "Lib/$sdkVersion/ucrt/x64"
} else {
    $sdkLocalRoot = Join-Path $projectRoot 'output/toolchain/windows-sdk'
    $sdkRoot = Join-Path $sdkLocalRoot 'microsoft.windows.sdk.cpp/c'
    $sdkVersion = Get-ChildItem -LiteralPath (Join-Path $sdkRoot 'Include') -Directory -ErrorAction SilentlyContinue | Sort-Object Name -Descending | Select-Object -First 1 -ExpandProperty Name
    if (-not $sdkVersion) { throw 'Windows SDK is missing. Run scripts/setup-windows-sdk.ps1 or install the SDK through Visual Studio.' }
    $sdkBin = Join-Path $sdkLocalRoot "microsoft.windows.sdk.buildtools/bin/$sdkVersion/x64"
    $sdkUm = Join-Path $sdkLocalRoot 'microsoft.windows.sdk.cpp.x64/c/um/x64'
    $sdkUcrt = Join-Path $sdkLocalRoot 'microsoft.windows.sdk.cpp.x64/c/ucrt/x64'
}
foreach ($required in @((Join-Path $msvcBin 'link.exe'), (Join-Path $msvcBin 'cl.exe'), (Join-Path $sdkBin 'rc.exe'), (Join-Path $sdkUm 'kernel32.lib'), (Join-Path $sdkUcrt 'ucrt.lib'))) {
    if (-not (Test-Path -LiteralPath $required)) { throw "Missing build dependency: $required" }
}

$env:PATH = "$cargoBin;$msvcBin;$sdkBin;$env:PATH"
$env:LIB = "$(Join-Path $msvcRoot 'lib/x64');$sdkUm;$sdkUcrt"
$env:INCLUDE = "$(Join-Path $msvcRoot 'include');$(Join-Path $sdkRoot "Include/$sdkVersion/ucrt");$(Join-Path $sdkRoot "Include/$sdkVersion/shared");$(Join-Path $sdkRoot "Include/$sdkVersion/um");$(Join-Path $sdkRoot "Include/$sdkVersion/winrt")"
$env:WindowsSdkDir = "$sdkRoot\"
$env:WindowsSDKVersion = "$sdkVersion\"
$env:WindowsSdkBinPath = "$(Split-Path $sdkBin)\"
$vcInstallRoot = if ($vsRoot) { Join-Path $vsRoot 'VC' } else { $portableVcRoot }
$env:VCINSTALLDIR = "$vcInstallRoot\"
$env:VCToolsInstallDir = "$msvcRoot\"
$env:VCToolsVersion = $msvcVersion
$env:VisualStudioVersion = '17.0'
$env:VSCMD_ARG_TGT_ARCH = 'x64'
$env:VSCMD_ARG_HOST_ARCH = 'x64'
$env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER = Join-Path $msvcBin 'link.exe'
$env:RC = Join-Path $sdkBin 'rc.exe'
$env:RC_PATH = $env:RC
if ($Action -eq 'Environment') { return }
# Keep builds predictable on PCs with limited RAM.
if (-not $env:CARGO_BUILD_JOBS) { $env:CARGO_BUILD_JOBS = '1' }

Push-Location $projectRoot
try {
    switch ($Action) {
        'Check' { & (Join-Path $cargoBin 'cargo.exe') check --locked --manifest-path Cargo.toml }
        'Build' { & (Join-Path $cargoBin 'cargo.exe') build --release --locked --manifest-path Cargo.toml }
    }
    if ($LASTEXITCODE -ne 0) { throw "Native $Action failed with exit code $LASTEXITCODE." }
} finally {
    Pop-Location
}
