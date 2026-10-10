param(
    [ValidateSet('Dev', 'Build', 'Check', 'Test', 'Validate')]
    [string]$Action = 'Dev'
)

$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
if (-not (Test-Path -LiteralPath $vswhere)) {
    throw 'Install Visual Studio C++ build tools first. See docs/WINDOWS.md.'
}
$vsRoot = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $vsRoot) { throw 'Visual Studio C++ x64 tools are missing.' }
$msvcVersion = (Get-Content -LiteralPath (Join-Path $vsRoot 'VC/Auxiliary/Build/Microsoft.VCToolsVersion.default.txt') -Raw).Trim()
$msvcRoot = Join-Path $vsRoot "VC/Tools/MSVC/$msvcVersion"
$msvcBin = Join-Path $msvcRoot 'bin/Hostx64/x64'
$cargoBin = Join-Path $env:USERPROFILE '.cargo/bin'
if (-not (Test-Path -LiteralPath (Join-Path $cargoBin 'cargo.exe'))) { throw 'Install Rust stable MSVC first.' }

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
$env:VCINSTALLDIR = "$(Join-Path $vsRoot 'VC')\"
$env:VCToolsInstallDir = "$msvcRoot\"
$env:VCToolsVersion = $msvcVersion
$env:VisualStudioVersion = '18.0'
$env:VSCMD_ARG_TGT_ARCH = 'x64'
$env:VSCMD_ARG_HOST_ARCH = 'x64'
$env:CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_LINKER = Join-Path $msvcBin 'link.exe'
$env:RC = Join-Path $sdkBin 'rc.exe'
$env:RC_PATH = $env:RC
# Keep builds predictable on PCs with limited RAM.
if (-not $env:CARGO_BUILD_JOBS) { $env:CARGO_BUILD_JOBS = '1' }

Push-Location $projectRoot
try {
    switch ($Action) {
        'Validate' {
            $smokeDir = Join-Path $projectRoot 'output/toolchain'
            New-Item -ItemType Directory -Path $smokeDir -Force | Out-Null
            $smokeSource = Join-Path $smokeDir 'smoke.rs'
            Set-Content -LiteralPath $smokeSource -Encoding utf8 -Value 'fn main() { println!("soundflow-rust-ready"); }'
            & (Join-Path $cargoBin 'rustc.exe') $smokeSource -o (Join-Path $smokeDir 'smoke.exe')
            if ($LASTEXITCODE -ne 0) { throw 'Rust/MSVC/SDK smoke compilation failed.' }
            & (Join-Path $smokeDir 'smoke.exe')
        }
        'Check' { & (Join-Path $cargoBin 'cargo.exe') check --locked --manifest-path Cargo.toml }
        'Test' { & (Join-Path $cargoBin 'cargo.exe') test --release --locked --manifest-path Cargo.toml }
        'Build' { & (Join-Path $cargoBin 'cargo.exe') build --release --locked --manifest-path Cargo.toml }
        'Dev' { & (Join-Path $cargoBin 'cargo.exe') run --locked --manifest-path Cargo.toml }
    }
    if ($LASTEXITCODE -ne 0) { throw "Native $Action failed with exit code $LASTEXITCODE." }
} finally {
    Pop-Location
}
