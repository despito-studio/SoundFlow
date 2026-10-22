param()

$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
$toolchainRoot = Join-Path $projectRoot 'output/toolchain'
$destination = [IO.Path]::GetFullPath((Join-Path $toolchainRoot 'msvc'))
$cache = Join-Path $toolchainRoot 'msvc-packages'
$packages = @(
    @{
        Name = 'Microsoft.VC.14.44.17.14.Tools.HostX64.TargetX64.base'
        Hash = 'f4ad3cee4ef18e9f36382399261de27be6910ab3467178d7e687377ec841306d'
        Url = 'https://download.visualstudio.microsoft.com/download/pr/aa209763-134c-427b-9960-9ca54ccc1734/f4ad3cee4ef18e9f36382399261de27be6910ab3467178d7e687377ec841306d/Microsoft.VC.14.44.17.14.Tools.HostX64.TargetX64.base.vsix'
    },
    @{
        Name = 'Microsoft.VC.14.44.17.14.Tools.HostX64.TargetX64.Res.base.enu'
        Hash = '603375e4d6d639bc6e0a7d0d657b17e0c3b72e52450f78eb41e987f40c3b5305'
        Url = 'https://download.visualstudio.microsoft.com/download/pr/aa209763-134c-427b-9960-9ca54ccc1734/603375e4d6d639bc6e0a7d0d657b17e0c3b72e52450f78eb41e987f40c3b5305/Microsoft.VC.14.44.17.14.Tools.HostX64.TargetX64.Res.base.enu.vsix'
    },
    @{
        Name = 'Microsoft.VC.14.44.17.14.CRT.Headers.base'
        Hash = '852382a9aa73502b7849c1bcadfb603ba7175c4e8b60e6aba03c7de711d4ece5'
        Url = 'https://download.visualstudio.microsoft.com/download/pr/c610cd8c-801b-44b8-a80a-82cc382aeb43/852382a9aa73502b7849c1bcadfb603ba7175c4e8b60e6aba03c7de711d4ece5/Microsoft.VC.14.44.17.14.CRT.Headers.base.vsix'
    },
    @{
        Name = 'Microsoft.VC.14.44.17.14.CRT.x64.Desktop.base'
        Hash = 'f01f701a7bcd9587a340898c851424f6a52bb913a70c185ff0d5bf0288c5831a'
        Url = 'https://download.visualstudio.microsoft.com/download/pr/67cf767c-5e71-47c2-a54a-cd5631e28942/f01f701a7bcd9587a340898c851424f6a52bb913a70c185ff0d5bf0288c5831a/Microsoft.VC.14.44.17.14.CRT.x64.Desktop.base.vsix'
    },
    @{
        Name = 'Microsoft.VC.14.44.17.14.CRT.x64.Store.base'
        Hash = '9135b03c0df53c7a0aa9bef7230a1c2ff4263a0ee7baa7e419d034f484f6bb56'
        Url = 'https://download.visualstudio.microsoft.com/download/pr/67cf767c-5e71-47c2-a54a-cd5631e28942/9135b03c0df53c7a0aa9bef7230a1c2ff4263a0ee7baa7e419d034f484f6bb56/Microsoft.VC.14.44.17.14.CRT.x64.Store.base.vsix'
    }
)

Add-Type -AssemblyName System.IO.Compression.FileSystem
New-Item -ItemType Directory -Path $cache,$destination -Force | Out-Null
foreach ($package in $packages) {
    $archive = Join-Path $cache ($package.Name + '.vsix')
    if (-not (Test-Path -LiteralPath $archive)) {
        Write-Output ('Downloading ' + $package.Name)
        Invoke-WebRequest -Uri $package.Url -OutFile ($archive + '.download') -UseBasicParsing
        Move-Item -LiteralPath ($archive + '.download') -Destination $archive
    }
    if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant() -ne $package.Hash) {
        throw ('MSVC package checksum failed: ' + $package.Name)
    }
    $marker = Join-Path $destination ('.soundflow-' + $package.Name)
    if (Test-Path -LiteralPath $marker) { continue }
    $zip = [IO.Compression.ZipFile]::OpenRead($archive)
    try {
        $safePrefix = $destination.TrimEnd('\') + '\'
        foreach ($entry in $zip.Entries) {
            if (-not $entry.FullName.StartsWith('Contents/', [StringComparison]::OrdinalIgnoreCase)) { continue }
            $relative = [Uri]::UnescapeDataString($entry.FullName.Substring(9))
            if (-not $relative) { continue }
            $target = [IO.Path]::GetFullPath((Join-Path $destination $relative))
            if (-not $target.StartsWith($safePrefix, [StringComparison]::OrdinalIgnoreCase)) { throw 'Unsafe MSVC package path.' }
            if (-not $entry.Name) { New-Item -ItemType Directory -Path $target -Force | Out-Null; continue }
            New-Item -ItemType Directory -Path (Split-Path $target) -Force | Out-Null
            [IO.Compression.ZipFileExtensions]::ExtractToFile($entry, $target, $true)
        }
    } finally { $zip.Dispose() }
    Set-Content -LiteralPath $marker -Value $package.Hash -Encoding ascii
    Write-Output ('Verified and extracted ' + $package.Name)
}

$compilerRoot = Get-ChildItem -LiteralPath (Join-Path $destination 'VC/Tools/MSVC') -Directory |
    Sort-Object Name -Descending | Select-Object -First 1
if (-not $compilerRoot) { throw 'The portable MSVC toolset was not extracted.' }
$compiler = Join-Path $compilerRoot.FullName 'bin/Hostx64/x64/cl.exe'
$linker = Join-Path $compilerRoot.FullName 'bin/Hostx64/x64/link.exe'
foreach ($required in @($compiler,$linker,(Join-Path $compilerRoot.FullName 'lib/x64/libcmt.lib'))) {
    if (-not (Test-Path -LiteralPath $required)) { throw ('Missing MSVC toolchain file: ' + $required) }
}
Write-Output ('Portable MSVC is ready: ' + $compilerRoot.FullName)
