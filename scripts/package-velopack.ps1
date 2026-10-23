param(
    [Parameter(Mandatory)][string]$Stage,
    [Parameter(Mandatory)][string]$Version,
    [string]$PreviousFeed,
    [switch]$SkipInstaller,
    [switch]$SkipZip,
    [string]$TermsUrl,
    [switch]$RequireTermsUrl
)
$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
if ($Version -notmatch '^\d+\.\d+\.\d+$') { throw 'A stable three-part version is required.' }
. (Join-Path $PSScriptRoot 'setup-velopack.ps1')
$build = Join-Path $projectRoot ("output/velopack/{0}-{1}" -f $Version,[guid]::NewGuid().ToString('N'))
$releases = Join-Path $build 'releases'
$history = Join-Path $build 'history'
$public = Join-Path $projectRoot "dist/release/$Version"
$pages = Join-Path $projectRoot 'dist/pages/updates/win-x64'
New-Item -ItemType Directory -Path $releases,$history,$public -Force | Out-Null
if ($PreviousFeed) {
    $base = [Uri]($PreviousFeed.TrimEnd('/') + '/')
    if ($base.Scheme -ne 'https' -or $base.UserInfo -or $base.Query -or $base.Fragment) { throw 'PreviousFeed must be an HTTPS directory URL.' }
    try {
        $feed = Invoke-RestMethod -Uri ([Uri]::new($base,'releases.win.json')) -TimeoutSec 30
    } catch {
        if ([int]$_.Exception.Response.StatusCode -ne 404) { throw }
        $feed = @{Assets=@()}
    }
    foreach ($asset in $feed.Assets) {
        if ($asset.PackageId -ne 'SoundFlow' -or $asset.Version -notmatch '^\d+\.\d+\.\d+$' -or
            $asset.Type -notin @('Full','Delta') -or $asset.FileName -cne "SoundFlow-$($asset.Version)-$($asset.Type.ToLowerInvariant()).nupkg" -or
            $asset.SHA256 -notmatch '^[a-fA-F0-9]{64}$' -or $asset.Size -lt 1 -or $asset.Size -gt 512MB) { throw 'Invalid previous update feed.' }
        if ([version]$asset.Version -ge [version]$Version) { throw "Version $Version must be newer than the published channel ($($asset.Version)). Never replace a public version." }
        $file = Join-Path $history $asset.FileName
        & curl.exe --fail --location --retry 3 --silent --show-error --output $file ([Uri]::new($base,$asset.FileName).AbsoluteUri)
        if ($LASTEXITCODE -ne 0) { throw "Could not restore $($asset.FileName) from Pages." }
        if ((Get-Item -LiteralPath $file).Length -ne $asset.Size -or (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash -ne $asset.SHA256) { throw 'Previous package integrity check failed.' }
        Copy-Item -LiteralPath $file -Destination $releases
    }
}
$arguments = @('pack','--packId','SoundFlow','--packVersion',$Version,'--packDir',(Resolve-Path -LiteralPath $Stage).Path,
    '--mainExe','SoundFlow.exe','--packTitle','SoundFlow','--packAuthors','SoundFlow','--channel','win','--runtime','win-x64',
    '--outputDir',$releases,'--icon',(Join-Path $projectRoot 'resources/icon.ico'),'--shortcuts','Desktop,StartMenu','--aumid','app.soundflow.native')
if ($SkipInstaller) { $arguments += '--noInst' }
# vpk requires one bootstrap format internally. In service-only builds its
# private portable byproduct stays in output; neither user download is published.
if ($SkipZip -and -not $SkipInstaller) { $arguments += '--noPortable' }
& $Vpk @arguments
if ($LASTEXITCODE -ne 0) { throw 'Velopack packaging failed.' }
& $Vpk upload local --path $history --outputDir $releases --channel win --keepMaxReleases 3 --regenerate
if ($LASTEXITCODE -ne 0) { throw 'Could not merge the Pages update channel.' }
# The Pages channel contains only service files. Setup and portable are independent.
$pages = [IO.Path]::GetFullPath($pages)
$allowed = [IO.Path]::GetFullPath((Join-Path $projectRoot 'dist/pages')) + [IO.Path]::DirectorySeparatorChar
if (-not $pages.StartsWith($allowed,[StringComparison]::OrdinalIgnoreCase)) { throw 'Invalid Pages output path.' }
if (Test-Path -LiteralPath $pages) {
    if ((Get-Item -LiteralPath $pages).Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Pages output must not be a junction.' }
    Remove-Item -LiteralPath $pages -Recurse -Force
}
New-Item -ItemType Directory -Path $pages -Force | Out-Null
Get-ChildItem -LiteralPath $history -File | Where-Object { $_.Extension -eq '.nupkg' -or $_.Name -eq 'releases.win.json' } | Copy-Item -Destination $pages
if (-not (Test-Path -LiteralPath (Join-Path $pages 'releases.win.json'))) { throw 'Missing Pages feed.' }
if (-not $SkipZip) { Copy-Item -LiteralPath (Join-Path $releases 'SoundFlow-win-Portable.zip') -Destination (Join-Path $public "SoundFlow-$Version-windows-x64.zip") -Force }
if (-not $SkipInstaller) {
    & (Join-Path $PSScriptRoot 'build-installer.ps1') -Payload (Join-Path $releases 'SoundFlow-win-Setup.exe') -Version $Version -OutputDirectory $public -TermsUrl $TermsUrl -RequireTermsUrl:$RequireTermsUrl
}
$expected = @("SoundFlow-$Version-Setup.exe","SoundFlow-$Version-windows-x64.zip")
if (@(Get-ChildItem -LiteralPath $public -File | Where-Object Name -notin $expected).Count) { throw 'The public release folder contains unexpected files.' }
Write-Host "Public release: $public"
Write-Host "Pages service channel: $pages"
Get-ChildItem -LiteralPath $public -File | Select-Object Name,Length
