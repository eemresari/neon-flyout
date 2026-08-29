# Pasif izleyici: HICBIR sentetik girdi gondermez. Sen sesi kendi normal
# yolunla (fare tusu / mixer / Spotify) degistirirken calisir ve Windows'un
# bir pencere yaratip yaratmadigini yakalar.
param([int]$Saniye = 45)

$src = @"
using System;
using System.Text;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class V {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll", SetLastError=true)] public static extern bool GetWindowBand(IntPtr h, out uint band);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L; public int T; public int R; public int B; }
  public static string Cls(IntPtr h){ var sb=new StringBuilder(256); GetClassName(h, sb, 256); return sb.ToString(); }
  public static string Txt(IntPtr h){ var sb=new StringBuilder(256); GetWindowTextW(h, sb, 256); return sb.ToString(); }
  public static List<IntPtr> All(){ var a=new List<IntPtr>(); EnumWindows((h,l)=>{a.Add(h);return true;}, IntPtr.Zero); return a; }
}
"@
Add-Type -TypeDefinition $src -Language CSharp

function Snap() {
  $d = @{}
  foreach ($h in [V]::All()) {
    $r = New-Object V+RECT; [void][V]::GetWindowRect($h, [ref]$r)
    $d[[int64]$h] = @{ vis = [V]::IsWindowVisible($h); rect = "$($r.L),$($r.T),$($r.R),$($r.B)" }
  }
  return $d
}
function Desc($k) {
  $h = [IntPtr]$k
  $p = 0; [void][V]::GetWindowThreadProcessId($h, [ref]$p)
  $pn = try { (Get-Process -Id $p -ErrorAction Stop).ProcessName } catch { "?" }
  $band = 0; [void][V]::GetWindowBand($h, [ref]$band)
  "0x{0:X} [{1}] pid={2}({3}) band={4} baslik='{5}'" -f $k, [V]::Cls($h), $p, $pn, $band, [V]::Txt($h)
}

$base = Snap
"[$(Get-Date -Format HH:mm:ss)] izleme basladi ($Saniye sn, $($base.Count) pencere). SIMDI fareyle sesi degistir."
$t0 = Get-Date
while (((Get-Date) - $t0).TotalSeconds -lt $Saniye) {
  $now = Snap
  foreach ($k in $now.Keys) {
    $ts = Get-Date -Format HH:mm:ss.fff
    if (-not $base.ContainsKey($k)) {
      "[$ts] YENI PENCERE  $(Desc $k)"; $base[$k] = $now[$k]
    } else {
      if ($base[$k].vis -ne $now[$k].vis) { "[$ts] GORUNURLUK $($base[$k].vis)->$($now[$k].vis)  $(Desc $k)"; $base[$k].vis = $now[$k].vis }
      if ($base[$k].rect -ne $now[$k].rect) { "[$ts] KONUM  $(Desc $k)"; $base[$k].rect = $now[$k].rect }
    }
  }
  Start-Sleep -Milliseconds 50
}
"[$(Get-Date -Format HH:mm:ss)] izleme bitti."
