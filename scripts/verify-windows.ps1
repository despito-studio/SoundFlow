param(
    [Alias('Executable')][string]$Exe,
    [string]$Resources,
    [string]$Output
)
$ErrorActionPreference = 'Stop'
$projectRoot = (Resolve-Path (Join-Path $PSScriptRoot '..')).Path
if (-not $Exe) { $Exe = Join-Path $projectRoot 'dist/windows-x64/SoundFlow.exe' }
if (-not $Output) { $Output = Join-Path $projectRoot 'output/native' }
$exePath = (Resolve-Path -LiteralPath $Exe).Path
if (-not $Resources) { $Resources = Split-Path -Parent $exePath }
$resourceRoot = (Resolve-Path -LiteralPath $Resources).Path
$outputRoot = [IO.Path]::GetFullPath($Output)
New-Item -ItemType Directory -Path $outputRoot -Force | Out-Null
$snapshotRoot = Join-Path $outputRoot 'screenshots'
New-Item -ItemType Directory -Path $snapshotRoot -Force | Out-Null
$nativeReport = Join-Path $outputRoot 'native-report.json'
$windowsReport = Join-Path $outputRoot 'windows-report.json'

if (-not ('SoundFlowVerification.Native' -as [type])) {
    Add-Type -TypeDefinition @'
using System;
using System.Collections.Generic;
using System.Runtime.InteropServices;
using System.Text;
namespace SoundFlowVerification {
  public sealed class ProcessEntry { public uint Id; public uint Parent; public string Name; }
  public sealed class WindowEntry { public IntPtr Handle; public string Title; }
  public static class Native {
    private delegate bool EnumWindow(IntPtr window, IntPtr data);
    [DllImport("user32.dll")] private static extern bool EnumWindows(EnumWindow callback, IntPtr data);
    [DllImport("user32.dll", CharSet=CharSet.Unicode)] private static extern int GetWindowText(IntPtr window, StringBuilder title, int count);
    [DllImport("user32.dll")] private static extern uint GetWindowThreadProcessId(IntPtr window, out uint process);
    [DllImport("user32.dll", SetLastError=true)] private static extern IntPtr SendMessageTimeout(IntPtr window, uint message, UIntPtr parameter, IntPtr data, uint flags, uint timeout, out UIntPtr result);
    [DllImport("kernel32.dll", SetLastError=true)] private static extern IntPtr CreateToolhelp32Snapshot(uint flags, uint process);
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)] private static extern bool Process32First(IntPtr snapshot, ref Entry entry);
    [DllImport("kernel32.dll", CharSet=CharSet.Unicode, SetLastError=true)] private static extern bool Process32Next(IntPtr snapshot, ref Entry entry);
    [DllImport("kernel32.dll")] private static extern bool CloseHandle(IntPtr handle);
    [StructLayout(LayoutKind.Sequential, CharSet=CharSet.Unicode)] private struct Entry {
      public uint Size; public uint Usage; public uint Id; public IntPtr Heap; public uint Module; public uint Threads; public uint Parent; public int Priority; public uint Flags;
      [MarshalAs(UnmanagedType.ByValTStr, SizeConst=260)] public string Name;
    }
    public static WindowEntry[] Windows(uint id) {
      var result=new List<WindowEntry>();
      EnumWindows((window,data)=>{ uint owner; GetWindowThreadProcessId(window,out owner); if(owner==id) {
        var title=new StringBuilder(512); GetWindowText(window,title,title.Capacity);
        if(title.Length>0) result.Add(new WindowEntry {Handle=window,Title=title.ToString()});
      } return true; },IntPtr.Zero); return result.ToArray();
    }
    public static ulong Icon(IntPtr window) {
      foreach(uint size in new uint[]{1,2,0}) { UIntPtr icon;
        if(SendMessageTimeout(window,0x007F,new UIntPtr(size),IntPtr.Zero,2,1000,out icon)!=IntPtr.Zero && icon!=UIntPtr.Zero) return icon.ToUInt64();
      } return 0;
    }
    public static ProcessEntry[] Processes() {
      var result=new List<ProcessEntry>(); IntPtr snapshot=CreateToolhelp32Snapshot(2,0);
      if(snapshot==new IntPtr(-1)) throw new InvalidOperationException("Could not inspect process tree.");
      try { var entry=new Entry {Size=(uint)Marshal.SizeOf(typeof(Entry))};
        if(Process32First(snapshot,ref entry)) do { result.Add(new ProcessEntry {Id=entry.Id,Parent=entry.Parent,Name=entry.Name}); } while(Process32Next(snapshot,ref entry));
      } finally {CloseHandle(snapshot);} return result.ToArray();
    }
  }
}
'@
}

function Quote-PathArgument([string]$Value) {
    if ($Value.Contains('"')) { throw 'A process path cannot contain a quotation mark.' }
    return '"' + ($Value -replace '(\\+)$', '$1$1') + '"'
}
function Start-OwnedProcess([string[]]$Arguments, [string]$Backend, [string]$LogName) {
    # Environment changes are scoped to the child launch and restored immediately.
    $environmentNames = @('APPDATA','LOCALAPPDATA','SLINT_BACKEND')
    $previous = @{}
    foreach ($name in $environmentNames) { $previous[$name] = [Environment]::GetEnvironmentVariable($name,'Process') }
    try {
        $env:APPDATA = Join-Path $sandbox 'Roaming'
        $env:LOCALAPPDATA = Join-Path $sandbox 'Local'
        if ($Backend) { $env:SLINT_BACKEND = $Backend }
        else { [Environment]::SetEnvironmentVariable('SLINT_BACKEND',$null,'Process') }
        $options = @{
            FilePath = $exePath; WorkingDirectory = $resourceRoot; WindowStyle = 'Hidden'; PassThru = $true
            RedirectStandardOutput = (Join-Path $outputRoot "$LogName.stdout.log")
            RedirectStandardError = (Join-Path $outputRoot "$LogName.stderr.log")
        }
        if ($Arguments.Count) { $options.ArgumentList = (($Arguments | ForEach-Object { Quote-PathArgument $_ }) -join ' ') }
        $child = Start-Process @options
        # Retain the native handle so Windows PowerShell can report an early
        # loader failure after the child has already exited.
        $null = $child.Handle
        return $child
    } finally {
        foreach ($name in $environmentNames) { [Environment]::SetEnvironmentVariable($name,$previous[$name],'Process') }
    }
}
function Get-Descendants([int]$Owner) {
    $all = @([SoundFlowVerification.Native]::Processes())
    $known = [Collections.Generic.HashSet[uint32]]::new()
    [void]$known.Add([uint32]$Owner)
    $children = [Collections.Generic.List[object]]::new()
    do {
        $added = $false
        foreach ($entry in $all) {
            if ($known.Contains($entry.Parent) -and -not $known.Contains($entry.Id)) {
                [void]$known.Add($entry.Id); $children.Add($entry); $added = $true
            }
        }
    } while ($added)
    return $children.ToArray()
}
function Assert-Children([int]$Owner, [hashtable]$Observed) {
    $children = @(Get-Descendants $Owner)
    foreach ($child in $children) { $Observed[[string]$child.Id] = $child.Name }
    $forbidden = @($children | Where-Object { $_.Name -match '(?i)python|webview|msedge|chrome|chromium|electron|ffmpeg' })
    if ($forbidden.Count) { throw "Unexpected browser/Python process: $($forbidden.Name -join ', ')" }
    $workers = @($children | Where-Object { $_.Name -ieq 'soundflow-node.exe' })
    if ($workers.Count -gt 1) { throw 'SoundFlow started more than one media worker.' }
    if (@($children | Where-Object { $_.Name -ieq 'node.exe' }).Count) { throw 'The package started an external system Node runtime.' }
    return $workers.Count
}
function Assert-ChildrenExited([hashtable]$Observed) {
    for ($attempt=0; $attempt -lt 30; $attempt++) {
        $alive = @([SoundFlowVerification.Native]::Processes() | Where-Object { $Observed.ContainsKey([string]$_.Id) -and $Observed[[string]$_.Id] -eq $_.Name })
        if (-not $alive.Count) { return $true }
        Start-Sleep -Milliseconds 100
    }
    throw "Owned child process survived SoundFlow exit: $($alive.Name -join ', ')"
}
function Read-Accessibility([IntPtr]$Handle) {
    try {
        Add-Type -AssemblyName UIAutomationClient,UIAutomationTypes -ErrorAction Stop
    } catch { return @{ status='unavailable'; reason='UI Automation assemblies are unavailable in this PowerShell runtime.' } }
    try {
        # FromHandle confines read-only inspection to this exact owned window.
        $element = [Windows.Automation.AutomationElement]::FromHandle($Handle)
        $elements = $element.FindAll([Windows.Automation.TreeScope]::Descendants,[Windows.Automation.Condition]::TrueCondition)
        $buttons = @(); $labels = @(); $unnamed = 0
        foreach ($entry in $elements) {
            $current = $entry.Current
            if ($current.ControlType -eq [Windows.Automation.ControlType]::Button) {
                if ($current.Name) { $buttons += $current.Name } else { $unnamed++ }
            } elseif ($current.ControlType -eq [Windows.Automation.ControlType]::Text -and $current.Name) { $labels += $current.Name }
        }
        return @{status='available';elements=$elements.Count;named_buttons=@($buttons | Select-Object -Unique);unnamed_buttons=$unnamed;labels=@($labels | Select-Object -Unique -First 30)}
    } catch { return @{status='inspection_failed';reason=$_.Exception.Message} }
}

$sandbox = Join-Path ([IO.Path]::GetTempPath()) ('soundflow-native-verify-' + [guid]::NewGuid().ToString('N'))
New-Item -ItemType Directory -Path (Join-Path $sandbox 'Roaming'),(Join-Path $sandbox 'Local') -Force | Out-Null
$report = [ordered]@{success=$false;checked_at=[DateTime]::UtcNow.ToString('o');executable=$exePath;resources=$resourceRoot;snapshot_backend='winit-software';ordinary_backend='default';native_report=$nativeReport}
$snapshotProcess = $null; $ordinaryProcess = $null; $duplicateProcess = $null; $forcedProcess = $null
$snapshotChildren = @{}; $ordinaryChildren = @{}; $forcedChildren = @{}
$failure = $null
try {
    $metadata = (Get-Item -LiteralPath $exePath).VersionInfo
    $report.executable_identity = @{product_name=$metadata.ProductName;description=$metadata.FileDescription;original_filename=$metadata.OriginalFilename;version=$metadata.ProductVersion}
    if ($metadata.ProductName -ne 'SoundFlow' -or $metadata.FileDescription -notlike 'SoundFlow*') { throw 'The native EXE is missing SoundFlow product identity.' }
    if (Test-Path -LiteralPath $nativeReport) { Remove-Item -LiteralPath $nativeReport }
    $snapshotProcess = Start-OwnedProcess @('--verify-native',$nativeReport,'--snapshot-dir',$snapshotRoot,'--resources',$resourceRoot) 'winit-software' 'snapshot'
    $deadline = [DateTime]::UtcNow.AddSeconds(90); $maxSnapshotWorkers = 0
    while (-not $snapshotProcess.HasExited) {
        $maxSnapshotWorkers = [Math]::Max($maxSnapshotWorkers,(Assert-Children $snapshotProcess.Id $snapshotChildren))
        if ([DateTime]::UtcNow -gt $deadline) { throw 'Native verification did not finish within 90 seconds.' }
        Start-Sleep -Milliseconds 150; $snapshotProcess.Refresh()
    }
    [void]$snapshotProcess.WaitForExit(1000)
    if ($snapshotProcess.ExitCode -ne 0) { throw "Native verification exited $($snapshotProcess.ExitCode)." }
    $report.snapshot_children_exited = Assert-ChildrenExited $snapshotChildren
    $report.snapshot_media_workers_max = $maxSnapshotWorkers
    if (-not (Test-Path -LiteralPath $nativeReport -PathType Leaf)) { throw 'Native verification did not write its JSON report.' }
    $native = Get-Content -LiteralPath $nativeReport -Raw -Encoding utf8 | ConvertFrom-Json
    if (-not $native.success) { throw "Native verification failed: $($native.error)" }
    if ($native.provider.engine -ne 'youtubei.js' -or -not $native.provider.runtime_bundled) { throw 'Native verification did not use the bundled JavaScript media engine.' }
    if (@($native.snapshots).Count -lt 8) { throw 'Native verification did not capture all eight UI snapshots.' }
    foreach ($snapshot in $native.snapshots) { if (-not (Test-Path -LiteralPath $snapshot.file -PathType Leaf)) { throw "Missing native screenshot: $($snapshot.file)" } }
    $report.native = $native

    # Exercise the Job Object with a real worker. An offline ordinary window
    # deliberately starts none, so stopping that window alone cannot prove it.
    $forcedReport = Join-Path $sandbox 'forced-report.json'
    $forcedProcess = Start-OwnedProcess @('--verify-native',$forcedReport,'--resources',$resourceRoot) $null 'forced-exit'
    $deadline = [DateTime]::UtcNow.AddSeconds(30); $forcedWorkers = 0
    while ($forcedWorkers -ne 1) {
        $forcedProcess.Refresh()
        if ($forcedProcess.HasExited) { throw 'The forced-exit verification process ended before its media worker was observed.' }
        $forcedWorkers = Assert-Children $forcedProcess.Id $forcedChildren
        if ([DateTime]::UtcNow -gt $deadline) { throw 'The forced-exit test did not observe a bundled media worker within 30 seconds.' }
        if ($forcedWorkers -ne 1) { Start-Sleep -Milliseconds 100 }
    }
    # Let native child registration finish before testing abrupt parent death.
    Start-Sleep -Milliseconds 100
    $forcedProcess.Refresh()
    if ($forcedProcess.HasExited -or (Assert-Children $forcedProcess.Id $forcedChildren) -ne 1) { throw 'The bundled media worker did not remain active for the forced-exit test.' }
    Stop-Process -Id $forcedProcess.Id -Force
    if (-not $forcedProcess.WaitForExit(10000)) { throw 'The exact owned forced-exit process did not stop.' }
    $report.forced_child_cleanup = @{
        parent_pid=$forcedProcess.Id;media_workers_observed=$forcedWorkers
        observed_descendants=@($forcedChildren.GetEnumerator() | ForEach-Object { @{pid=[int]$_.Key;name=$_.Value} })
        children_exited=(Assert-ChildrenExited $forcedChildren)
    }

    $ordinaryArguments = @('--data-dir',(Join-Path $sandbox 'native'),'--offline')
    $ordinaryProcess = Start-OwnedProcess $ordinaryArguments $null 'startup'
    $window = $null; $deadline = [DateTime]::UtcNow.AddSeconds(30); $maxOrdinaryWorkers = 0
    while (-not $window) {
        $ordinaryProcess.Refresh()
        if ($ordinaryProcess.HasExited) { throw "Ordinary SoundFlow startup exited early ($($ordinaryProcess.ExitCode))." }
        $maxOrdinaryWorkers = [Math]::Max($maxOrdinaryWorkers,(Assert-Children $ordinaryProcess.Id $ordinaryChildren))
        $window = [SoundFlowVerification.Native]::Windows([uint32]$ordinaryProcess.Id) | Where-Object Title -eq 'SoundFlow' | Select-Object -First 1
        if ([DateTime]::UtcNow -gt $deadline) { throw 'The ordinary native SoundFlow window did not appear within 30 seconds.' }
        if (-not $window) { Start-Sleep -Milliseconds 150 }
    }
    $icon = [SoundFlowVerification.Native]::Icon($window.Handle)
    if (-not $icon) { throw 'The SoundFlow window did not expose its own icon through WM_GETICON.' }
    $report.ordinary = @{pid=$ordinaryProcess.Id;window_title=$window.Title;window_handle=$window.Handle.ToInt64();window_icon=$icon;accessibility=(Read-Accessibility $window.Handle)}
    if ($report.ordinary.accessibility.status -ne 'available') { throw "Native accessibility inspection failed: $($report.ordinary.accessibility.reason)" }
    if ($report.ordinary.accessibility.elements -lt 10) { throw 'The native window did not expose a meaningful accessibility tree.' }
    if ($report.ordinary.accessibility.unnamed_buttons -gt 0) { throw 'The native window exposed an unnamed accessible button.' }

    $duplicateProcess = Start-OwnedProcess $ordinaryArguments $null 'duplicate'
    if (-not $duplicateProcess.WaitForExit(10000)) { throw 'A duplicate launch did not activate the existing app and exit.' }
    $ordinaryProcess.Refresh()
    $sameWindow = @([SoundFlowVerification.Native]::Windows([uint32]$ordinaryProcess.Id) | Where-Object { $_.Handle -eq $window.Handle -and $_.Title -eq 'SoundFlow' }).Count -eq 1
    if ($duplicateProcess.ExitCode -ne 0 -or $ordinaryProcess.HasExited -or -not $sameWindow) { throw 'Duplicate activation did not preserve the original SoundFlow window/PID.' }
    $report.duplicate_activation = @{exit_code=$duplicateProcess.ExitCode;same_pid=$true;same_window=$sameWindow}
    for ($sample=0; $sample -lt 10; $sample++) {
        $maxOrdinaryWorkers = [Math]::Max($maxOrdinaryWorkers,(Assert-Children $ordinaryProcess.Id $ordinaryChildren))
        Start-Sleep -Milliseconds 100
    }
    $report.ordinary.media_workers_max = $maxOrdinaryWorkers
    $report.ordinary.observed_descendants = @($ordinaryChildren.GetEnumerator() | ForEach-Object { @{pid=[int]$_.Key;name=$_.Value} })
    # Closing an ordinary window can hide it to the tray; stop only our own PID.
    # Worker cleanup is separately proven above with a non-empty process tree.
    Stop-Process -Id $ordinaryProcess.Id -Force
    if (-not $ordinaryProcess.WaitForExit(10000)) { throw 'The exact owned SoundFlow process did not stop.' }
    $report.shutdown = @{mode='owned-process termination';children_exited=(Assert-ChildrenExited $ordinaryChildren);snapshot_graceful_shutdown=$report.snapshot_children_exited;forced_worker_exit=$report.forced_child_cleanup.children_exited}
    $report.success = $true
} catch { $failure = $_; $report.error = $_.Exception.Message }
finally {
    # Never enumerate arbitrary process names for termination.
    foreach ($owned in @($snapshotProcess,$forcedProcess,$duplicateProcess,$ordinaryProcess)) {
        if ($owned) { $owned.Refresh(); if (-not $owned.HasExited) { Stop-Process -Id $owned.Id -Force -ErrorAction SilentlyContinue; [void]$owned.WaitForExit(10000) } }
    }
    $resolvedSandbox = [IO.Path]::GetFullPath($sandbox)
    $expectedSandbox = [IO.Path]::GetFullPath((Join-Path ([IO.Path]::GetTempPath()) (Split-Path -Leaf $sandbox)))
    if ($resolvedSandbox -eq $expectedSandbox -and (Split-Path -Leaf $resolvedSandbox) -match '^soundflow-native-verify-[a-f0-9]{32}$') { Remove-Item -LiteralPath $resolvedSandbox -Recurse -Force -ErrorAction SilentlyContinue }
    $report | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath $windowsReport -Encoding utf8
}
if ($failure) { throw $failure }
Write-Output "Native Windows checks passed: $windowsReport"
