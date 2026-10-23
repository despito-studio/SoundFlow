param([Parameter(Mandatory=$true)][string]$OutputPath)
$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$checkoutCargoRoot = Join-Path $projectRoot 'output/toolchain/cargo'
if (-not $env:CARGO_HOME -and (Test-Path -LiteralPath (Join-Path $checkoutCargoRoot 'bin/cargo.exe'))) {
    $env:CARGO_HOME = $checkoutCargoRoot
    $env:RUSTUP_HOME = Join-Path $projectRoot 'output/toolchain/rustup'
}
$cargoPath = if ($env:CARGO_HOME) { Join-Path $env:CARGO_HOME 'bin/cargo.exe' } else { Join-Path $env:USERPROFILE '.cargo/bin/cargo.exe' }
if (-not (Test-Path -LiteralPath $cargoPath)) {
    $cargoCommand = Get-Command cargo.exe -ErrorAction SilentlyContinue | Select-Object -First 1
    if (-not $cargoCommand) { throw 'Cargo was not found. See docs/WINDOWS.md.' }
    $cargoPath = $cargoCommand.Source
}
$metadataText = & $cargoPath metadata --manifest-path (Join-Path $projectRoot 'Cargo.toml') --format-version 1 --locked --filter-platform x86_64-pc-windows-msvc
if ($LASTEXITCODE -ne 0) { throw 'Could not collect dependency license metadata.' }
$metadata = $metadataText | ConvertFrom-Json
$sections = [Collections.Generic.List[string]]::new()
$sections.Add('SoundFlow native Windows third-party notices. The license text and attribution for each shipped dependency follow.')
foreach ($package in ($metadata.packages | Where-Object source | Sort-Object name,version)) {
    $sections.Add("$($package.name) $($package.version)`n$($package.repository)`nLicense expression: $($package.license)")
    $packageFolder = Split-Path $package.manifest_path
    $licenseFiles = @(Get-ChildItem -LiteralPath $packageFolder -File -Recurse | Where-Object {
        $_.Name -match '(^|[._-])(LICENSE|LICENCE|COPYING|COPYRIGHT|NOTICE)([._-]|$)' -or $_.Name -match '^(OFL|UFL)\.txt$'
    })
    if ($package.license_file) {
        $declaredLicense = Join-Path $packageFolder $package.license_file
        if (Test-Path -LiteralPath $declaredLicense) { $licenseFiles += Get-Item -LiteralPath $declaredLicense }
    }
    foreach ($file in ($licenseFiles | Sort-Object FullName -Unique)) {
        $relativeFile = $file.FullName.Substring($packageFolder.Length).TrimStart('\','/')
        $sections.Add("$relativeFile`n$([IO.File]::ReadAllText($file.FullName))")
    }
}
$absoluteOutput = [IO.Path]::GetFullPath($OutputPath)
[IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($absoluteOutput)) | Out-Null
[IO.File]::WriteAllText($absoluteOutput, ($sections -join "`n`n--------------------`n`n"), [Text.UTF8Encoding]::new($false))
