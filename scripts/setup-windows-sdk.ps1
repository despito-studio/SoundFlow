param([string]$Version = '10.0.26100.9169')

$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$sdkCache = Join-Path $projectRoot 'output/toolchain/windows-sdk'
$hashes = @{
    'microsoft.windows.sdk.cpp' = '475269434dcd808a67853773272f972c3229c0e10c3ddc821290e70cc0f6904d'
    'microsoft.windows.sdk.cpp.x64' = 'df6226a051e320942abfbd57848b43d18772996ecd66beadad240f2a56ed2f7b'
    'microsoft.windows.sdk.buildtools' = '6000c971fc9155052a8359779b30b6682c39e091664d83b3852c4702ab6d238e'
}
if ($Version -ne '10.0.26100.9169') { throw 'Add the official package checksums before changing the SDK version.' }
Add-Type -AssemblyName System.IO.Compression.FileSystem
New-Item -ItemType Directory -Path $sdkCache -Force | Out-Null
foreach ($package in $hashes.Keys) {
    $archive = Join-Path $sdkCache "$package.$Version.nupkg"
    $destination = [IO.Path]::GetFullPath((Join-Path $sdkCache $package))
    if (-not (Test-Path -LiteralPath $archive)) {
        $temporary = "$archive.download"
        Invoke-WebRequest -Uri "https://api.nuget.org/v3-flatcontainer/$package/$Version/$package.$Version.nupkg" -OutFile $temporary
        Move-Item -LiteralPath $temporary -Destination $archive -Force
    }
    if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant() -ne $hashes[$package]) {
        throw "Windows SDK package checksum failed: $package"
    }
    $marker = Join-Path $destination '.soundflow-sdk-version'
    if ((Test-Path -LiteralPath $marker) -and (Get-Content -LiteralPath $marker -Raw).Trim() -eq $Version) {
        Write-Output "$package $Version verified"
        continue
    }
    $zip = [IO.Compression.ZipFile]::OpenRead($archive)
    try {
        $prefix = $destination.TrimEnd('\') + '\'
        foreach ($entry in $zip.Entries) {
            $target = [IO.Path]::GetFullPath((Join-Path $destination $entry.FullName))
            if (-not $target.StartsWith($prefix, [StringComparison]::OrdinalIgnoreCase)) { throw 'Unsafe SDK package path.' }
            if (-not $entry.Name) { New-Item -ItemType Directory -Path $target -Force | Out-Null; continue }
            New-Item -ItemType Directory -Path (Split-Path $target) -Force | Out-Null
            [IO.Compression.ZipFileExtensions]::ExtractToFile($entry, $target, $true)
        }
    } finally { $zip.Dispose() }
    Set-Content -LiteralPath $marker -Value $Version -Encoding ascii
    Write-Output "$package $Version extracted and verified"
}
