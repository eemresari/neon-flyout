$src = @"
using System;
using System.Text;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class W {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll", SetLastError=true)] public static extern bool GetWindowBand(IntPtr h, out uint band);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L; public int T; public int R; public int B; }
  public static string Cls(IntPtr h){ var sb=new StringBuilder(256); GetClassName(h, sb, 256); return sb.ToString(); }
  public static string Txt(IntPtr h){ var sb=new StringBuilder(256); GetWindowTextW(h, sb, 256); return sb.ToString(); }
  public static List<IntPtr> All(){
    var acc = new List<IntPtr>();
    EnumWindows((h,l) => { acc.Add(h); return true; }, IntPtr.Zero);
    return acc;
  }
}
"@
Add-Type -TypeDefinition $src -Language CSharp

function Snap() {
  $d = @{}
  foreach ($h in [W]::All()) {
    $r = New-Object W+RECT; [void][W]::GetWindowRect($h, [ref]$r)
    $d[[int64]$h] = @{ vis = [W]::IsWindowVisible($h); rect = "$($r.L),$($r.T),$($r.R),$($r.B)"; cls = [W]::Cls($h) }
  }
  return $d
}
function Desc($k) {
  $h = [IntPtr]$k
  $pid2 = 0; [void][W]::GetWindowThreadProcessId($h, [ref]$pid2)
  $pname = try { (Get-Process -Id $pid2 -ErrorAction Stop).ProcessName } catch { "?" }
  $band = 0; [void][W]::GetWindowBand($h, [ref]$band)
  "0x{0:X} [{1}] pid={2}({3}) band={4} baslik='{5}'" -f $k, [W]::Cls($h), $pid2, $pname, $band, [W]::Txt($h)
}

$base = Snap
"Taban: $($base.Count) pencere. VOLUME_UP gonderiliyor, 2 sn izleniyor..."
[W]::keybd_event(0xAF, 0, 0, [UIntPtr]::Zero); [W]::keybd_event(0xAF, 0, 2, [UIntPtr]::Zero)

$olaylar = New-Object System.Collections.ArrayList
$t0 = Get-Date
while (((Get-Date) - $t0).TotalMilliseconds -lt 2000) {
  $now = Snap
  foreach ($k in $now.Keys) {
    if (-not $base.ContainsKey($k)) {
      [void]$olaylar.Add("YENI PENCERE   $(Desc $k)")
      $base[$k] = $now[$k]
    } else {
      if ($base[$k].vis -ne $now[$k].vis) {
        [void]$olaylar.Add("GORUNURLUK $($base[$k].vis)->$($now[$k].vis)  $(Desc $k)")
        $base[$k].vis = $now[$k].vis
      }
      if ($base[$k].rect -ne $now[$k].rect) {
        [void]$olaylar.Add("KONUM $($base[$k].rect) -> $($now[$k].rect)  $(Desc $k)")
        $base[$k].rect = $now[$k].rect
      }
    }
  }
  Start-Sleep -Milliseconds 60
}

""
"== 2 sn icindeki tum pencere olaylari =="
if ($olaylar.Count -eq 0) { "  (hicbir sey degismedi)" } else { $olaylar | ForEach-Object { "  $_" } }

Start-Sleep -Milliseconds 500
[W]::keybd_event(0xAE, 0, 0, [UIntPtr]::Zero); [W]::keybd_event(0xAE, 0, 2, [UIntPtr]::Zero)
""
"(ses seviyesi geri alindi)"
