param(
    [Parameter(Mandatory = $true)][string]$Out
)

# Brings the TimeTrek window to the front and saves a screenshot of just that window.
Add-Type -AssemblyName System.Drawing
Add-Type @"
using System; using System.Runtime.InteropServices;
public class TtShot {
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool SetForegroundWindow(IntPtr h);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int c);
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte sc, uint fl, UIntPtr ex);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L, T, R, B; }
}
"@

[TtShot]::SetProcessDPIAware() | Out-Null
$p = Get-Process timetrek -ErrorAction SilentlyContinue | Where-Object { $_.MainWindowHandle -ne 0 } | Select-Object -First 1
if (-not $p) { Write-Error 'TimeTrek has no visible window (is it hidden in the tray?)'; exit 1 }

# A tap of Alt lets us take the foreground from another process.
[TtShot]::keybd_event(0x12, 0, 0, [UIntPtr]::Zero)
[TtShot]::ShowWindow($p.MainWindowHandle, 9) | Out-Null
[TtShot]::SetForegroundWindow($p.MainWindowHandle) | Out-Null
[TtShot]::keybd_event(0x12, 0, 2, [UIntPtr]::Zero)
Start-Sleep -Seconds 2

$r = New-Object TtShot+RECT
[TtShot]::GetWindowRect($p.MainWindowHandle, [ref]$r) | Out-Null
$bmp = New-Object System.Drawing.Bitmap ($r.R - $r.L), ($r.B - $r.T)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($r.L, $r.T, 0, 0, $bmp.Size)
$bmp.Save($Out)
Write-Output "Saved $Out ($($bmp.Width)x$($bmp.Height))"
