param()
$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$toolRoot = Join-Path $projectRoot 'output/toolchain'
$dotnetRoot = Join-Path $toolRoot 'dotnet'
$dotnet = Join-Path $dotnetRoot 'dotnet.exe'
$sdkVersion = '8.0.426'
$vpkVersion = '1.2.161'
$env:DOTNET_ROOT = $dotnetRoot
$env:DOTNET_CLI_HOME = Join-Path $toolRoot 'dotnet-home'
$env:DOTNET_SKIP_FIRST_TIME_EXPERIENCE = '1'
$env:DOTNET_CLI_TELEMETRY_OPTOUT = '1'
$env:DOTNET_NOLOGO = '1'
if (-not (Test-Path -LiteralPath (Join-Path $dotnetRoot "sdk/$sdkVersion"))) {
    New-Item -ItemType Directory -Path $dotnetRoot -Force | Out-Null
    $archive = Join-Path $toolRoot "dotnet-sdk-$sdkVersion.zip"
    $hash = '10261657109f1c62126f106a9ac966739b025b5f6b4a9f88ab7c6e73cacce9aec771895daa3267b8da933a83c32ee91bf458dc6704e1ddcd9bcf0d5edd036abb'
    if (-not (Test-Path -LiteralPath $archive)) {
        & curl.exe --fail --location --retry 3 --output $archive "https://builds.dotnet.microsoft.com/dotnet/Sdk/$sdkVersion/dotnet-sdk-$sdkVersion-win-x64.zip"
        if ($LASTEXITCODE -ne 0) { throw 'Could not download the build-only .NET SDK.' }
    }
    if ((Get-FileHash -LiteralPath $archive -Algorithm SHA512).Hash.ToLowerInvariant() -ne $hash) { throw 'The .NET SDK checksum did not match Microsoft release metadata.' }
    Expand-Archive -LiteralPath $archive -DestinationPath $dotnetRoot -Force
}
$vpkRoot = Join-Path $toolRoot 'velopack'
$script:Vpk = Join-Path $vpkRoot 'vpk.exe'
if (-not (Test-Path -LiteralPath $script:Vpk)) {
    & $dotnet tool install vpk --version $vpkVersion --framework net8.0 --tool-path $vpkRoot --add-source 'https://api.nuget.org/v3/index.json'
    if ($LASTEXITCODE -ne 0) { throw 'Velopack packaging tool installation failed.' }
}
$actual = & $dotnet tool list --tool-path $vpkRoot
if ($LASTEXITCODE -ne 0 -or "$actual" -notmatch [regex]::Escape($vpkVersion)) { throw "Expected vpk $vpkVersion; found $actual" }
