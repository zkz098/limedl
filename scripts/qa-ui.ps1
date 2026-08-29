param(
    [string]$Action,        # comma-separated steps: "click:x,y", "type:text", "shot:path", "move:w,h", "wait:ms"
    [int]$WinW = 2240,
    [int]$WinH = 1400
)
$ErrorActionPreference = "Stop"
Add-Type -MemberDefinition @'
[DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
[DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr hWnd);
[DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr hWnd, int x, int y, int w, int h, bool repaint);
[DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
[DllImport("user32.dll")] public static extern void mouse_event(uint dwFlags, uint dx, uint dy, uint dwData, UIntPtr dwExtraInfo);
[DllImport("user32.dll")] public static extern void keybd_event(byte bVk, byte bScan, uint dwFlags, UIntPtr dwExtraInfo);
'@ -Name WinQ -Namespace U32
[U32.WinQ]::SetProcessDPIAware() | Out-Null
Add-Type -AssemblyName System.Drawing

function Click([int]$x, [int]$y) {
    [U32.WinQ]::SetCursorPos($x, $y) | Out-Null
    Start-Sleep -Milliseconds 200
    [U32.WinQ]::mouse_event(2, 0, 0, 0, [UIntPtr]::Zero)
    [U32.WinQ]::mouse_event(4, 0, 0, 0, [UIntPtr]::Zero)
}
function Shot([string]$path) {
    $vs = [System.Windows.Forms.SystemInformation]::VirtualScreen
    $bmp = New-Object System.Drawing.Bitmap($vs.Width, $vs.Height)
    $g = [System.Drawing.Graphics]::FromImage($bmp)
    $g.CopyFromScreen($vs.X, $vs.Y, 0, 0, $bmp.Size)
    $bmp.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    $g.Dispose(); $bmp.Dispose()
}
Add-Type -AssemblyName System.Windows.Forms

$proc = Get-Process limedl-native -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $proc) { Write-Host "limedl-native not running"; exit 1 }
[U32.WinQ]::SetForegroundWindow($proc.MainWindowHandle) | Out-Null
[U32.WinQ]::MoveWindow($proc.MainWindowHandle, 0, 0, $WinW, $WinH, $true) | Out-Null
Start-Sleep -Milliseconds 800

foreach ($step in $Action -split '\|') {
    $parts = $step -split ':', 2
    switch ($parts[0]) {
        "click" { $xy = $parts[1] -split ','; Click ([int]$xy[0]) ([int]$xy[1]); Start-Sleep -Milliseconds 400 }
        "dbl"   { $xy = $parts[1] -split ','; Click ([int]$xy[0]) ([int]$xy[1]); Click ([int]$xy[0]) ([int]$xy[1]); Start-Sleep -Milliseconds 300 }
        "type"  { [System.Windows.Forms.SendKeys]::SendWait($parts[1]); Start-Sleep -Milliseconds 300 }
        "key"   { [System.Windows.Forms.SendKeys]::SendWait($parts[1]); Start-Sleep -Milliseconds 300 }
        "shot"  { Shot $parts[1]; Write-Host "saved $($parts[1])" }
        "wait"  { Start-Sleep -Milliseconds ([int]$parts[1]) }
    }
}
