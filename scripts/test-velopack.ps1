param()
$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
. (Join-Path $PSScriptRoot 'build-windows.ps1') -Action Environment
. (Join-Path $PSScriptRoot 'setup-velopack.ps1')
& cargo build --locked --release --example update_probe
if ($LASTEXITCODE -ne 0) { throw 'Update probe compilation failed.' }
$root = Join-Path $projectRoot ('output/update-integration/' + [guid]::NewGuid().ToString('N'))
$stage = Join-Path $root 'stage'
$feed = Join-Path $root 'feed'
$portable = Join-Path $root 'portable'
New-Item -ItemType Directory -Path $stage,$feed,$portable -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $projectRoot 'target/release/examples/update_probe.exe') -Destination (Join-Path $stage 'SoundFlowProbe.exe')
[IO.File]::WriteAllText((Join-Path $stage 'fixture.txt'),'original fixture')
& $Vpk pack -u SoundFlowProbe -v 1.0.0 -p $stage -e SoundFlowProbe.exe -o $feed --channel win --runtime win-x64 --noInst
if ($LASTEXITCODE -ne 0) { throw 'Initial probe package failed.' }
Expand-Archive -LiteralPath (Join-Path $feed 'SoundFlowProbe-win-Portable.zip') -DestinationPath $portable
# Seed the installed-version cache so this run also exercises the real delta patcher.
New-Item -ItemType Directory -Path (Join-Path $portable 'packages') -Force | Out-Null
Copy-Item -LiteralPath (Join-Path $feed 'SoundFlowProbe-1.0.0-full.nupkg') -Destination (Join-Path $portable 'packages')
[IO.File]::WriteAllText((Join-Path $stage 'fixture.txt'),'updated fixture')
& $Vpk pack -u SoundFlowProbe -v 1.0.1 -p $stage -e SoundFlowProbe.exe -o $feed --channel win --runtime win-x64 --noInst
if ($LASTEXITCODE -ne 0) { throw 'Update-only probe package failed.' }
# Deliberately leave ONLY service packages/feed: no Setup or portable can be used.
Get-ChildItem -LiteralPath $feed -File | Where-Object Extension -in @('.zip','.exe') | Remove-Item -Force
# Keep the feed entry but remove target Full: a hidden SDK fallback must fail,
# so a passing result proves that the real delta patcher was used.
Remove-Item -LiteralPath (Join-Path $feed 'SoundFlowProbe-1.0.1-full.nupkg') -Force
$sentinel = Join-Path $root 'user-data.txt'
[IO.File]::WriteAllText($sentinel,'personal library fixture')
$result = Join-Path $root 'result.txt'
$arguments = @('--feed',('"' + $feed + '"'),'--result',('"' + $result + '"'))
$process = Start-Process -FilePath (Join-Path $portable 'SoundFlowProbe.exe') -ArgumentList $arguments -WorkingDirectory $root -WindowStyle Hidden -PassThru
$deadline = [DateTime]::UtcNow.AddSeconds(90)
while (-not (Test-Path -LiteralPath $result) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 200 }
if (-not (Test-Path -LiteralPath $result)) { throw "Apply/restart timed out. Fixtures and framework logs retained at $root" }
$text = Get-Content -LiteralPath $result -Raw
if (-not $text.StartsWith('PASS:')) { throw "$text (fixtures: $root)" }
if ((Get-Content -LiteralPath $sentinel -Raw) -ne 'personal library fixture') { throw 'Data fixture changed.' }
Write-Host "$text`nEvidence: $root"
