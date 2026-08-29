$src = @"
using System;
using System.Text;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class Q {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindowExW(IntPtr p, IntPtr c, string cls, string win);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll", SetLastError=true)] public static extern bool GetWindowBand(IntPtr h, out uint band);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L; public int T; public int R; public int B; }
  public static string Cls(IntPtr h){ var sb=new StringBuilder(256); GetClassName(h, sb, 256); return sb.ToString(); }
  public static string Txt(IntPtr h){ var sb=new StringBuilder(256); GetWindowTextW(h, sb, 256); return sb.ToString(); }
  public static List<IntPtr> Visible(){
    var acc = new List<IntPtr>();
    EnumWindows((h,l) => { if (IsWindowVisible(h)) acc.Add(h); return true; }, IntPtr.Zero);
    return acc;
  }
}
"@
Add-Type -TypeDefinition $src -Language CSharp

function Show-Win($h) {
  $pid2 = 0; [void][Q]::GetWindowThreadProcessId($h, [ref]$pid2)
  $pname = try { (Get-Process -Id $pid2 -ErrorAction Stop).ProcessName } catch { "?" }
  $band = 0; [void][Q]::GetWindowBand($h, [ref]$band)
  $r = New-Object Q+RECT; [void][Q]::GetWindowRect($h, [ref]$r)
  $child = [Q]::FindWindowExW($h, [IntPtr]::Zero, $null, $null)
  $cc = if ($child -ne [IntPtr]::Zero) { [Q]::Cls($child) } else { "-" }
  "  0x{0:X} [{1}] pid={2}({3}) band={4} rect={5},{6} {7}x{8} baslik='{9}' ilkCocuk='{10}'" -f `
    [int64]$h, [Q]::Cls($h), $pid2, $pname, $band, $r.L, $r.T, ($r.R-$r.L), ($r.B-$r.T), [Q]::Txt($h), $cc
}

$before = [Q]::Visible()
"Once gorunur pencere: $($before.Count)"

# VOLUME_UP (0xAF) -> OSD tetiklensin
[Q]::keybd_event(0xAF, 0, 0, [UIntPtr]::Zero)
[Q]::keybd_event(0xAF, 0, 2, [UIntPtr]::Zero)
Start-Sleep -Milliseconds 350

$after = [Q]::Visible()
"Sonra gorunur pencere: $($after.Count)"
""
"== Ses tusuyla GORUNUR HALE GELEN pencereler =="
$yeni = $after | Where-Object { $before -notcontains $_ }
if ($yeni) { foreach ($h in $yeni) { Show-Win $h } } else { "  (yok)" }

# geri al
Start-Sleep -Milliseconds 1200
[Q]::keybd_event(0xAE, 0, 0, [UIntPtr]::Zero)
[Q]::keybd_event(0xAE, 0, 2, [UIntPtr]::Zero)
""
"(ses seviyesi geri alindi)"
