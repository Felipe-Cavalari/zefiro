param(
    [ValidateSet('SuspendExisting','RestoreExisting','Start','Open','Capture','Key','Click','RightClick','Drag','Cycles','Close','Stop')][string]$Action = 'Start',
    [string]$Exe = 'target/debug/zefiro.exe',
    [int]$X = 40, [int]$Y = 20, [int]$Key = 0,
    [int]$EndX = 1800, [int]$EndY = 20, [int]$Cycles = 50,
    [string]$Out = '.superpowers/sdd/2026-10-15-fase-1b-modo-de-edicao/editor.png'
)
$ErrorActionPreference = 'Stop'
Add-Type @'
using System;
using System.Runtime.InteropServices;
public class ZefiroEditorProbe {
  [StructLayout(LayoutKind.Sequential)] public struct Point { public int X, Y; }
  [DllImport("user32.dll")] public static extern bool GetCursorPos(out Point p);
  [DllImport("user32.dll")] public static extern IntPtr WindowFromPoint(Point p);
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x,int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f,uint x,uint y,uint d,UIntPtr e);
  [DllImport("user32.dll")] public static extern void keybd_event(byte k,byte s,uint f,UIntPtr e);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h,out uint p);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h,uint m,IntPtr w,IntPtr l);
  [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h,System.Text.StringBuilder s,int n);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc p,IntPtr l);
  public delegate bool EnumProc(IntPtr h,IntPtr l);
  public static IntPtr Bar(uint pid) { IntPtr found=IntPtr.Zero; EnumWindows((h,l)=>{ uint p; GetWindowThreadProcessId(h,out p); var c=new System.Text.StringBuilder(100); GetClassName(h,c,100); if(p==pid&&c.ToString()=="ZefiroBar")found=h; return true; },IntPtr.Zero); return found; }
  public static int WindowCount(uint pid) { int count=0; EnumWindows((h,l)=>{ uint p; GetWindowThreadProcessId(h,out p); if(p==pid)count++; return true; },IntPtr.Zero); return count; }
}
'@
$root = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$runtime = Join-Path $root '.superpowers/sdd/2026-10-15-fase-1b-modo-de-edicao/runtime'
$processFile = Join-Path $runtime 'bar.pid'
if ($Action -eq 'SuspendExisting') {
    New-Item -ItemType Directory -Force $runtime | Out-Null
    $existing = @(Get-Process zefiro -ErrorAction SilentlyContinue)
    if ($existing.Count -ne 1) { throw 'Esperada exatamente uma barra existente para suspender.' }
    @{ Path = $existing[0].Path } | ConvertTo-Json | Set-Content (Join-Path $runtime 'existing-bar.json')
    $bar = [ZefiroEditorProbe]::Bar([uint32]$existing[0].Id)
    if ($bar -eq [IntPtr]::Zero) { throw 'Não encontrei a janela ZefiroBar do processo existente.' }
    [void][ZefiroEditorProbe]::PostMessage($bar,0x10,[IntPtr]::Zero,[IntPtr]::Zero)
    if (-not $existing[0].WaitForExit(5000)) { throw 'A barra existente não encerrou; não foi forçada.' }
    Write-Output 'Barra anterior encerrada normalmente; caminho guardado para restauração.'
    exit
}
if ($Action -eq 'RestoreExisting') {
    if (Get-Process zefiro -ErrorAction SilentlyContinue) { throw 'Ainda existe uma barra; finalize o teste antes de restaurar.' }
    $saved = Get-Content (Join-Path $runtime 'existing-bar.json') | ConvertFrom-Json
    if (-not (Test-Path -LiteralPath $saved.Path -PathType Leaf)) { throw 'Executável anterior ausente.' }
    Start-Process -FilePath $saved.Path -WindowStyle Hidden
    Write-Output 'Barra anterior reaberta.'
    exit
}
if ($Action -eq 'Start') {
    if (Get-Process zefiro -ErrorAction SilentlyContinue) { throw 'Já existe uma barra em execução; não vou substituí-la automaticamente.' }
    New-Item -ItemType Directory -Force $runtime | Out-Null
    if (-not (Test-Path (Join-Path $runtime 'config.toml'))) {
        "theme = 'obsidian'`n[island]`nenabled = false`n" | Set-Content (Join-Path $runtime 'config.toml') -Encoding utf8
    }
    $env:ZEFIRO_CONFIG_DIR = $runtime
    $probeProcess = Start-Process -FilePath (Join-Path $root $Exe) -PassThru -WindowStyle Hidden
    $probeProcess.Id | Set-Content $processFile
    Start-Sleep -Milliseconds 600
    Write-Output "Barra de teste: PID $($probeProcess.Id); configuração isolada em $runtime"
    exit
}
$probeId = [uint32](Get-Content $processFile)
$process = Get-Process -Id $probeId -ErrorAction Stop
if ($process.ProcessName -ne 'zefiro') { throw 'O PID salvo não pertence ao Zefiro.' }
if ($Action -eq 'Stop') {
    $bar = [ZefiroEditorProbe]::Bar($probeId)
    if ($bar -eq [IntPtr]::Zero) { throw 'Nenhuma ZefiroBar encontrada.' }
    [void][ZefiroEditorProbe]::PostMessage($bar,0x10,[IntPtr]::Zero,[IntPtr]::Zero)
    $process.WaitForExit(5000) | Out-Null
    if (-not $process.HasExited) { throw 'A barra não encerrou no prazo; não foi forçada.' }
    exit
}
if ($Action -eq 'Capture') {
    Add-Type -AssemblyName System.Drawing
    $bitmap = [System.Drawing.Bitmap]::new(520,730)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    try { $graphics.CopyFromScreen(0,0,0,0,$bitmap.Size); $bitmap.Save((Join-Path $root $Out)) } finally { $graphics.Dispose(); $bitmap.Dispose() }
    Write-Output (Join-Path $root $Out)
    exit
}
$before = [ZefiroEditorProbe+Point]::new()
[void][ZefiroEditorProbe]::GetCursorPos([ref]$before)
try {
    if ($Action -eq 'Click' -or $Action -eq 'Drag') {
        $point = [ZefiroEditorProbe+Point]::new(); $point.X = $X; $point.Y = $Y
        $targetId = [uint32]0
        [void][ZefiroEditorProbe]::GetWindowThreadProcessId([ZefiroEditorProbe]::WindowFromPoint($point),[ref]$targetId)
        if ($targetId -ne $probeId) { throw "O alvo do clique não pertence à barra de teste; nenhum clique enviado ($X,$Y)." }
    }
    if ($Action -eq 'Drag') {
        [void][ZefiroEditorProbe]::SetCursorPos($X,$Y)
        [ZefiroEditorProbe]::mouse_event(2,0,0,0,[UIntPtr]::Zero)
        try {
            for ($step = 1; $step -le 20; $step++) {
                [void][ZefiroEditorProbe]::SetCursorPos([int]($X + ($EndX-$X)*$step/20),[int]($Y + ($EndY-$Y)*$step/20))
                Start-Sleep -Milliseconds 16
            }
        } finally { [ZefiroEditorProbe]::mouse_event(4,0,0,0,[UIntPtr]::Zero) }
        exit
    }
    if ($Action -eq 'Cycles') {
        $initialWindows = [ZefiroEditorProbe]::WindowCount($probeId)
        $process.Refresh()
        $initialPrivate = $process.PrivateMemorySize64
        for ($cycle = 0; $cycle -lt $Cycles; $cycle++) {
            [void][ZefiroEditorProbe]::SetCursorPos($X,$Y)
            [ZefiroEditorProbe]::mouse_event(8,0,0,0,[UIntPtr]::Zero)
            [ZefiroEditorProbe]::mouse_event(16,0,0,0,[UIntPtr]::Zero)
            Start-Sleep -Milliseconds 100
            $foregroundId = [uint32]0
            [void][ZefiroEditorProbe]::GetWindowThreadProcessId([ZefiroEditorProbe]::GetForegroundWindow(),[ref]$foregroundId)
            if ($foregroundId -ne $probeId) { throw "Ciclo $cycle perdeu o foco; nenhuma tecla enviada a outro app." }
            foreach ($virtualKey in @(40,40,40,40,13)) {
                [ZefiroEditorProbe]::keybd_event([byte]$virtualKey,0,0,[UIntPtr]::Zero)
                [ZefiroEditorProbe]::keybd_event([byte]$virtualKey,0,2,[UIntPtr]::Zero)
                Start-Sleep -Milliseconds 15
            }
            Start-Sleep -Milliseconds 100
            [ZefiroEditorProbe]::keybd_event(27,0,0,[UIntPtr]::Zero)
            [ZefiroEditorProbe]::keybd_event(27,0,2,[UIntPtr]::Zero)
            Start-Sleep -Milliseconds 100
        }
        Start-Sleep -Milliseconds 800
        $process.Refresh()
        $finalWindows = [ZefiroEditorProbe]::WindowCount($probeId)
        if ($initialWindows -ne $finalWindows) { throw "Janelas cresceram: $initialWindows → $finalWindows" }
        [pscustomobject]@{ Cycles=$Cycles; WindowsBefore=$initialWindows; WindowsAfter=$finalWindows; PrivateBeforeMB=$initialPrivate/1MB; PrivateAfterMB=$process.PrivateMemorySize64/1MB } | ConvertTo-Json | Tee-Object -FilePath (Join-Path $runtime 'editor-cycles.json')
        exit
    }
    if ($Action -eq 'Open' -or $Action -eq 'Click' -or $Action -eq 'RightClick') {
        [void][ZefiroEditorProbe]::SetCursorPos($X,$Y)
        [ZefiroEditorProbe]::mouse_event($(if ($Action -eq 'RightClick' -or $Action -eq 'Open') { 8 } else { 2 }),0,0,0,[UIntPtr]::Zero)
        [ZefiroEditorProbe]::mouse_event($(if ($Action -eq 'RightClick' -or $Action -eq 'Open') { 16 } else { 4 }),0,0,0,[UIntPtr]::Zero)
        Start-Sleep -Milliseconds 200
    }
    if ($Action -eq 'Key' -or $Action -eq 'Close' -or $Action -eq 'Open') {
        $foreground = [ZefiroEditorProbe]::GetForegroundWindow()
        $foregroundId = [uint32]0
        [void][ZefiroEditorProbe]::GetWindowThreadProcessId($foreground,[ref]$foregroundId)
        if ($foregroundId -ne $probeId) {
            $foregroundClass = [System.Text.StringBuilder]::new(120)
            [void][ZefiroEditorProbe]::GetClassName($foreground,$foregroundClass,120)
            throw "A janela em primeiro plano ($foregroundId / $foregroundClass) não é a barra de teste ($probeId); nenhuma tecla foi enviada."
        }
        if ($Action -eq 'Open') { $keys = @(40,40,40,40,13) }
        elseif ($Action -eq 'Close') { $keys = @(27) }
        else { $keys = @($Key) }
        foreach ($virtualKey in $keys) {
            [ZefiroEditorProbe]::keybd_event([byte]$virtualKey,0,0,[UIntPtr]::Zero)
            [ZefiroEditorProbe]::keybd_event([byte]$virtualKey,0,2,[UIntPtr]::Zero)
            Start-Sleep -Milliseconds 40
        }
    }
} finally { [void][ZefiroEditorProbe]::SetCursorPos($before.X,$before.Y) }
