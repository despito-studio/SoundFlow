# Публікація релізу SoundFlow на GitHub.
# Токен вводиться приховано і не зберігається на диску.
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $MyInvocation.MyCommand.Path
$git = Join-Path $root 'output\toolchain\mingit\cmd\git.exe'
$repo = 'despito-studio/SoundFlow'
$tag = 'v3.5.0'
$zip = Join-Path $root 'dist\SoundFlow-3.5.0-windows-x64.zip'
$sums = Join-Path $root 'dist\SHA256SUMS-3.5.0.txt'
$notes = Join-Path $root 'docs\RELEASE-3.5.0.md'

if (-not (Test-Path -LiteralPath $zip)) { throw "Немає архіву: $zip" }

Write-Host 'SoundFlow 3.5.0 -> GitHub release' -ForegroundColor Cyan
Write-Host ''
Write-Host 'Потрібен classic token зі scope "repo" (Settings -> Developer settings -> Tokens).'
Write-Host 'Він вводиться приховано і нікуди не записується.'
$secure = Read-Host 'GitHub token' -AsSecureString
$bstr = [Runtime.InteropServices.Marshal]::SecureStringToBSTR($secure)
try { $token = [Runtime.InteropServices.Marshal]::PtrToStringAuto($bstr) } finally { [Runtime.InteropServices.Marshal]::ZeroFreeBSTR($bstr) }
if ([string]::IsNullOrWhiteSpace($token)) { throw 'Порожній токен' }

[Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
$headers = @{ Authorization = "Bearer $token"; Accept = 'application/vnd.github+json'; 'User-Agent' = 'SoundFlow' }
$remote = "https://x-access-token:$token@github.com/$repo.git"
$env:GIT_TERMINAL_PROMPT = '0'

Write-Host 'Перевіряю токен...'
$user = Invoke-RestMethod -Uri 'https://api.github.com/user' -Headers $headers
Write-Host ("  користувач: " + $user.login)

Write-Host 'Пушу main і тег...'
& $git -C $root push --quiet $remote main
if ($LASTEXITCODE -ne 0) { throw 'Не вдалося запушити main' }
& $git -C $root push --quiet $remote $tag
if ($LASTEXITCODE -ne 0) { throw 'Не вдалося запушити тег' }

Write-Host 'Створюю реліз...'
$release = $null
try {
    $release = Invoke-RestMethod -Uri "https://api.github.com/repos/$repo/releases/tags/$tag" -Headers $headers
    Write-Host '  реліз уже існує, оновлюю опис'
    $payload = @{ name = 'SoundFlow 3.5.0'; body = (Get-Content -LiteralPath $notes -Raw); draft = $false; prerelease = $false } | ConvertTo-Json -Depth 3
    $release = Invoke-RestMethod -Method Patch -Uri "https://api.github.com/repos/$repo/releases/$($release.id)" -Headers $headers -Body ([Text.Encoding]::UTF8.GetBytes($payload)) -ContentType 'application/json; charset=utf-8'
} catch {
    $payload = @{ tag_name = $tag; name = 'SoundFlow 3.5.0'; body = (Get-Content -LiteralPath $notes -Raw); draft = $false; prerelease = $false } | ConvertTo-Json -Depth 3
    $release = Invoke-RestMethod -Method Post -Uri "https://api.github.com/repos/$repo/releases" -Headers $headers -Body ([Text.Encoding]::UTF8.GetBytes($payload)) -ContentType 'application/json; charset=utf-8'
}

$existing = @()
try { $existing = (Invoke-RestMethod -Uri "https://api.github.com/repos/$repo/releases/$($release.id)/assets" -Headers $headers) | ForEach-Object { $_.name } } catch {}

foreach ($asset in @($zip, $sums)) {
    if (-not (Test-Path -LiteralPath $asset)) { continue }
    $name = Split-Path -Leaf $asset
    if ($existing -contains $name) { Write-Host ("  " + $name + " вже завантажено"); continue }
    Write-Host ("Завантажую " + $name + "...")
    Invoke-RestMethod -Method Post -Uri "https://uploads.github.com/repos/$repo/releases/$($release.id)/assets?name=$name" -Headers $headers -ContentType 'application/octet-stream' -InFile $asset | Out-Null
}

Write-Host ''
Write-Host ("[OK] Реліз опубліковано: " + $release.html_url) -ForegroundColor Green
Read-Host 'Натисни Enter, щоб закрити'
