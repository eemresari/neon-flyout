$src = @"
using System;
using System.Text;
using System.Collections.Generic;
using System.Runtime.InteropServices;
public static class P {
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc cb, IntPtr l);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetClassName(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern int GetWindowTextW(IntPtr h, StringBuilder s, int n);
  [DllImport("user32.dll", CharSet=CharSet.Unicode)] public static extern IntPtr FindWindowExW(IntPtr p, IntPtr c, string cls, string win);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll", SetLastError=true)] public static extern bool GetWindowBand(IntPtr h, out uint band);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int L; public int T; public int R; public int B; }
  public static string Cls(IntPtr h){ var sb=new StringBuilder(256); GetClassName(h, sb, 256); return sb.ToString(); }
  public static string Txt(IntPtr h){ var sb=new StringBuilder(256); GetWindowTextW(h, sb, 256); return sb.ToString(); }
  public static List<IntPtr> Top(string cls){
    var acc = new List<IntPtr>();
    EnumWindows((h,l) => { if (Cls(h) == cls) acc.Add(h); return true; }, IntPtr.Zero);
    return acc;
  }
}
"@
Add-Type -TypeDefinition $src -Language CSharp

"Windows build: $([Environment]::OSVersion.Version.Build)"
""

foreach ($outer in @("XamlExplorerHostIslandWindow", "NativeHWNDHost")) {
  $list = [P]::Top($outer)
  "== $outer : $($list.Count) adet =="
  foreach ($h in $list) {
    $pid2 = 0; [void][P]::GetWindowThreadProcessId($h, [ref]$pid2)
    $pname = try { (Get-Process -Id $pid2 -ErrorAction Stop).ProcessName } catch { "?" }
    $band = 0; $bandOk = [P]::GetWindowBand($h, [ref]$band)
    $r = New-Object P+RECT; [void][P]::GetWindowRect($h, [ref]$r)
    $vis = [P]::IsWindowVisible($h)

    $bridge = [P]::FindWindowExW($h, [IntPtr]::Zero, "Windows.UI.Composition.DesktopWindowContentBridge", "DesktopWindowXamlSource")
    $dui    = [P]::FindWindowExW($h, [IntPtr]::Zero, "DirectUIHWND", "")
    $inp    = [IntPtr]::Zero
    if ($bridge -ne [IntPtr]::Zero) { $inp = [P]::FindWindowExW($bridge, [IntPtr]::Zero, "Windows.UI.Input.InputSite.WindowClass", $null) }

    $rect = "{0},{1} {2}x{3}" -f $r.L, $r.T, ($r.R - $r.L), ($r.B - $r.T)
    "  hwnd=0x{0:X} pid={1}({2}) band={3}{4} gorunur={5} baslik='{6}'" -f `
      [int64]$h, $pid2, $pname, $band, $(if(-not $bandOk){" (GetWindowBand BASARISIZ)"}else{""}), $vis, [P]::Txt($h)
    "       rect=$rect  bridge=$(if($bridge -ne 0){'VAR'}else{'yok'})  DirectUIHWND=$(if($dui -ne 0){'VAR'}else{'yok'})  InputSite=$(if($inp -ne 0){'VAR'}else{'yok'})"
  }
  ""
}
