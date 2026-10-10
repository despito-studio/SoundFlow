param([Parameter(Mandatory=$true)][string]$OutputPath)
$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$cargoPath = Join-Path $env:USERPROFILE '.cargo/bin/cargo.exe'
$metadataText = & $cargoPath metadata --manifest-path (Join-Path $projectRoot 'Cargo.toml') --format-version 1 --locked --filter-platform x86_64-pc-windows-msvc
if ($LASTEXITCODE -ne 0) { throw 'Could not collect dependency license metadata.' }
$metadata = $metadataText | ConvertFrom-Json
$sections = [Collections.Generic.List[string]]::new()
$sections.Add('SoundFlow native Windows third-party notices. Slint is used under its Royalty-free Applications License 2.0; see SLINT-LICENSE.md and About SoundFlow.')
foreach ($package in ($metadata.packages | Where-Object source | Sort-Object name,version)) {
    $sections.Add("$($package.name) $($package.version)`n$($package.repository)`nLicense expression: $($package.license)")
    $packageFolder = Split-Path $package.manifest_path
    $licenseFiles = @(Get-ChildItem -LiteralPath $packageFolder -File | Where-Object { $_.Name -match '^(LICENSE|COPYING|COPYRIGHT|NOTICE)([._-]|$)' })
    if ($package.license_file) {
        $declaredLicense = Join-Path $packageFolder $package.license_file
        if (Test-Path -LiteralPath $declaredLicense) { $licenseFiles += Get-Item -LiteralPath $declaredLicense }
    }
    $licenseFolder = Join-Path $packageFolder 'LICENSES'
    if (Test-Path -LiteralPath $licenseFolder) { $licenseFiles += Get-ChildItem -LiteralPath $licenseFolder -File }
    foreach ($file in ($licenseFiles | Sort-Object FullName -Unique)) {
        $sections.Add("$($file.Name)`n$([IO.File]::ReadAllText($file.FullName))")
    }
}
[IO.File]::WriteAllText([IO.Path]::GetFullPath($OutputPath), ($sections -join "`n`n--------------------`n`n"), [Text.UTF8Encoding]::new($false))
