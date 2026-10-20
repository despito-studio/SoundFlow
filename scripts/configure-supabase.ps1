param(
    [Parameter(Mandatory = $true)][string]$Url,
    [Parameter(Mandatory = $true)][string]$AnonKey,
    [string]$GoogleClientId = '',
    [string]$GoogleClientSecret = '',
    [string]$DataDir = ''
)

$ErrorActionPreference = 'Stop'

if ($Url -notmatch '^https://[a-z0-9-]+\.supabase\.co/?$') {
    throw "Очікується https://<project>.supabase.co, отримано: $Url"
}
if ($AnonKey -match '^sb_secret_' -or $AnonKey -match 'service_role') {
    throw 'Це секретний ключ (service_role / sb_secret_) — його не можна використовувати в застосунку.'
}
if ($AnonKey -notmatch '^(sb_publishable_[A-Za-z0-9_-]+|ey[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.)') {
    throw 'Очікується publishable-ключ (sb_publishable_…) або anon JWT (eyJ…).'
}

if (-not $DataDir) { $DataDir = Join-Path $env:APPDATA 'app.soundflow.desktop' }
New-Item -ItemType Directory -Force -Path $DataDir | Out-Null
$configPath = Join-Path $DataDir 'cloud.json'

$config = [ordered]@{}
if (Test-Path -LiteralPath $configPath) {
    $existing = Get-Content -LiteralPath $configPath -Raw | ConvertFrom-Json
    foreach ($property in $existing.PSObject.Properties) { $config[$property.Name] = $property.Value }
}

$config['supabase_url'] = $Url.TrimEnd('/')
$config['supabase_anon_key'] = $AnonKey.Trim()
if ($GoogleClientId) { $config['google_client_id'] = $GoogleClientId.Trim() }
if ($GoogleClientSecret) { $config['google_client_secret'] = $GoogleClientSecret.Trim() }

$json = $config | ConvertTo-Json -Depth 5
[IO.File]::WriteAllText($configPath, $json, [Text.UTF8Encoding]::new($false))

Write-Host "[OK] Записано $configPath" -ForegroundColor Green
Write-Host ("     Supabase: " + $config['supabase_url'])
Write-Host ("     Google client: " + $(if ($config['google_client_id']) { $config['google_client_id'] } else { '(типовий, вбудований)' }))
