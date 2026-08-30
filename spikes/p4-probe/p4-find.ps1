# FindWindowEx tabanli sonda. p4-probe.ps1'den farki bu ve fark KRITIK:
# EnumWindows yuksek z-band'deki pencereleri (band 18 = AboveLockUX) listelemiyor,
# FindWindowEx buluyor. Native flyout host'u tam olarak orada yasiyor.
$src = @"
using System;
using System.Text;
using System.Runtime.InteropServices;
public static class F {
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindowExW(IntPtr p, IntPtr c, string cls, string win);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll", SetLastError=true)] public static extern bool GetWindowBand(IntPtr h, out uint band);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool IsIconic(IntPtr h);
  [DllImport("user32.dll")] public static extern int GetWindowLongW(IntPtr h, int i);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L; public int T; public int R; public int B; }
  public static string Txt(IntPtr h){ var sb=new StringBuilder(256); GetWindowTextW(h, sb, 256); return sb.ToString(); }
}
"@
Add-Type -TypeDefinition $src -Language CSharp

$WS_EX_LAYERED = 0x00080000; $WS_EX_TRANSPARENT = 0x00000020; $GWL_EXSTYLE = -20
$build = [Environment]::OSVersion.Version.Build
$outer = if ($build -ge 22620) { "XamlExplorerHostIslandWindow" } else { "NativeHWNDHost" }
$innerCls = if ($build -ge 22620) { "Windows.UI.Composition.DesktopWindowContentBridge" } else { "DirectUIHWND" }
$innerTtl = if ($build -ge 22620) { "DesktopWindowXamlSource" } else { "" }

"build=$build  dis sinif=$outer"
""
$h = [IntPtr]::Zero
$n = 0
while (($h = [F]::FindWindowExW([IntPtr]::Zero, $h, $outer, "")) -ne [IntPtr]::Zero) {
  $n++
  $inner = [F]::FindWindowExW($h, [IntPtr]::Zero, $innerCls, $innerTtl)
  $p = 0; [void][F]::GetWindowThreadProcessId($h, [ref]$p)
  $pn = try { (Get-Process -Id $p -ErrorAction Stop).ProcessName } catch { "?" }
  $band = 0; $bandOk = [F]::GetWindowBand($h, [ref]$band)
  $r = New-Object F+RECT; [void][F]::GetWindowRect($h, [ref]$r)
  $ex = [F]::GetWindowLongW($h, $GWL_EXSTYLE)
  $bayrak = @()
  if ($ex -band $WS_EX_LAYERED) { $bayrak += "LAYERED" }
  if ($ex -band $WS_EX_TRANSPARENT) { $bayrak += "TRANSPARENT" }

  "aday #$n  hwnd=0x{0:X} pid={1}({2})" -f [int64]$h, $p, $pn
  "   band={0}{1}  ic pencere={2}" -f $band, $(if(-not $bandOk){" (okunamadi)"}else{""}), $(if($inner -ne [IntPtr]::Zero){"VAR"}else{"yok"})
  "   rect={0},{1} {2}x{3}  gorunur={4} min={5}  exstyle=0x{6:X8} {7}" -f `
    $r.L, $r.T, ($r.R-$r.L), ($r.B-$r.T), [F]::IsWindowVisible($h), [F]::IsIconic($h), $ex, ($bayrak -join "+")
  "   baslik='{0}'" -f [F]::Txt($h)
  if ($band -eq 18) {
    # Konuma BAKMA: SW_MINIMIZE, SetWindowPos'un -99999'unu ezip pencereyi
    # -32768,-32768'e (klasik minimize konumu) alıyor. Parmak izi bayraklar.
    $bastirilmis = ($ex -band $WS_EX_LAYERED) -and ($ex -band $WS_EX_TRANSPARENT) -and [F]::IsIconic($h)
    if ($bastirilmis) { "   >>> BASTIRILMIS (layered+transparent+minimize)" } else { "   >>> normal durumda" }
  }
  ""
}
if ($n -eq 0) { "(hic aday bulunamadi)" }
