param([switch]$DryRun)

$ErrorActionPreference = 'Stop'
$root    = Split-Path -Parent $MyInvocation.MyCommand.Path
$src     = Join-Path $root 'output\portable\SoundFlow.exe'
$dstDir  = 'C:\SoundFlow'
$dst     = Join-Path $dstDir 'SoundFlow.exe'
$bakDir  = Join-Path $root 'backups'

function Fail($message) {
    Write-Host ''
    Write-Host "[X] $message" -ForegroundColor Red
    Write-Host ''
    Read-Host 'Натисни Enter, щоб закрити'
    exit 1
}

Write-Host 'SoundFlow — оновлення встановленої версії' -ForegroundColor Cyan
Write-Host ''

if (-not (Test-Path -LiteralPath $src)) { Fail "Не знайдено новий білд: $src" }
if (-not (Test-Path -LiteralPath $dst)) { Fail "Не знайдено встановлений застосунок: $dst" }

$srcInfo = Get-Item -LiteralPath $src
Write-Host ('Новий білд : ' + $srcInfo.Length + ' байт, ' + $srcInfo.LastWriteTime)
Write-Host ('Куди       : ' + $dst)

if ($DryRun) {
    Write-Host ''
    Write-Host '[DryRun] Перевірка шляхів успішна, нічого не змінено.' -ForegroundColor Yellow
    exit 0
}

Get-Process -Name SoundFlow -ErrorAction SilentlyContinue | ForEach-Object {
    Write-Host 'Закриваю запущений SoundFlow...'
    $_ | Stop-Process -Force
}
Start-Sleep -Milliseconds 800

if (-not (Test-Path -LiteralPath $bakDir)) { New-Item -ItemType Directory -Path $bakDir -Force | Out-Null }
$stamp = Get-Date -Format 'yyyyMMdd-HHmmss'
$backup = Join-Path $bakDir ('SoundFlow-before-update-' + $stamp + '.exe')
try {
    Copy-Item -LiteralPath $dst -Destination $backup -Force
    Write-Host ('Резервна копія: ' + $backup)
} catch {
    Write-Host ('[!] Не вдалося зробити резервну копію: ' + $_.Exception.Message) -ForegroundColor Yellow
}

Write-Host 'Копіюю новий білд...'
try {
    Copy-Item -LiteralPath $src -Destination $dst -Force
} catch {
    Fail ('Не вдалося записати ' + $dst + '. Запусти цей файл від імені адміністратора (правою кнопкою). ' + $_.Exception.Message)
}

$newHash = (Get-FileHash -LiteralPath $dst -Algorithm SHA256).Hash
$srcHash = (Get-FileHash -LiteralPath $src -Algorithm SHA256).Hash
if ($newHash -ne $srcHash) { Fail 'Файл скопійовано, але хеш не збігається. Спробуй ще раз.' }

$desktop = [Environment]::GetFolderPath('Desktop')
$lnkPath = Join-Path $desktop 'SoundFlow.lnk'
try {
    $shell = New-Object -ComObject WScript.Shell
    $needShortcut = $true
    if (Test-Path -LiteralPath $lnkPath) {
        $existing = $shell.CreateShortcut($lnkPath)
        if ($existing.TargetPath -eq $dst) { $needShortcut = $false }
    }
    if ($needShortcut) {
        $lnk = $shell.CreateShortcut($lnkPath)
        $lnk.TargetPath = $dst
        $lnk.WorkingDirectory = $dstDir
        $lnk.IconLocation = $dst + ',0'
        $lnk.Save()
        Write-Host 'Ярлик на робочому столі оновлено.'
    } else {
        Write-Host 'Ярлик на робочому столі вже вказує на потрібний файл.'
    }
} catch {
    Write-Host ('[!] Ярлик не оновлено: ' + $_.Exception.Message) -ForegroundColor Yellow
}

Write-Host ''
Write-Host '[OK] Готово. Запускаю SoundFlow...' -ForegroundColor Green
Start-Process -FilePath $dst -WorkingDirectory $dstDir
Start-Sleep -Seconds 2
Write-Host 'Далі запускай звичайним ярликом — там уже нова версія.'
Write-Host ''
Read-Host 'Натисни Enter, щоб закрити'
