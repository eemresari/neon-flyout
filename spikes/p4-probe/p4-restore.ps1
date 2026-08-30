# Native flyout'u ELLE geri al.
#
# Uygulamanin restore()'u yalnizca duzgun kapanista (RunEvent::Exit) calisiyor.
# Surec oldurulurse (Ctrl+C, gorev yoneticisi, cokme) kabugun kutusu bastirilmis
# kalir. Bu betik onu geri koyar: WS_EX_LAYERED|WS_EX_TRANSPARENT temizlenir,
# pencere 0,0'a alinir ve SW_RESTORE edilir.
$src = @"
using System;
using System.Runtime.InteropServices;
public static class R {
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindowExW(IntPtr p, IntPtr c, string cls, string win);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll", SetLastError=true)] public static extern bool GetWindowBand(IntPtr h, out uint band);
  [DllImport("user32.dll")] public static extern int GetWindowLongW(IntPtr h, int i);
  [DllImport("user32.dll")] public static extern int SetWindowLongW(IntPtr h, int i, int v);
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int cx, int cy, uint flags);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
}
"@
Add-Type -TypeDefinition $src -Language CSharp

$GWL_EXSTYLE = -20; $WS_EX_LAYERED = 0x00080000; $WS_EX_TRANSPARENT = 0x00000020
$SWP = 0x0001 -bor 0x0004 -bor 0x0010   # NOSIZE | NOZORDER | NOACTIVATE
$SW_RESTORE = 9

$build = [Environment]::OSVersion.Version.Build
$outer = if ($build -ge 22620) { "XamlExplorerHostIslandWindow" } else { "NativeHWNDHost" }

$h = [IntPtr]::Zero
$bulundu = 0
while (($h = [R]::FindWindowExW([IntPtr]::Zero, $h, $outer, "")) -ne [IntPtr]::Zero) {
  $band = 0; [void][R]::GetWindowBand($h, [ref]$band)
  if ($band -ne 18) { continue }
  $p = 0; [void][R]::GetWindowThreadProcessId($h, [ref]$p)
  $pn = try { (Get-Process -Id $p -ErrorAction Stop).ProcessName } catch { "" }
  if ($pn -ne "explorer") { continue }

  $bulundu++
  $ex = [R]::GetWindowLongW($h, $GWL_EXSTYLE)
  $yeni = $ex -band (-bnot ($WS_EX_LAYERED -bor $WS_EX_TRANSPARENT))
  [void][R]::SetWindowLongW($h, $GWL_EXSTYLE, $yeni)
  [void][R]::SetWindowPos($h, [IntPtr]::Zero, 0, 0, 0, 0, $SWP)
  [void][R]::ShowWindow($h, $SW_RESTORE)
  "geri alindi: hwnd=0x{0:X} exstyle 0x{1:X8} -> 0x{2:X8}" -f [int64]$h, $ex, $yeni
}
if ($bulundu -eq 0) { "band 18'de explorer'a ait pencere bulunamadi (bastirma zaten yok olabilir)" }
