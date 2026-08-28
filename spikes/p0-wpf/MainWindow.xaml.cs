// P0 gecikme kapısı — WPF tarafı.
//
// Bu ATILACAK koddur (ROADMAP P0).
//
// ÜÇ SİNYAL ÖLÇÜLÜR, ÜÇÜ DE AYNI t0'A GÖRE:
//
//   raf1 / raf2 — CompositionTarget.Rendering'in gösterimden sonraki 1. ve 2.
//                 ateşlemesi. ÖLÇÜT DEĞİLDİR: WPF'te bir Rendering handler'ı
//                 bağlıyken MediaContext pencere gizli olsa da tikleyip durur
//                 (idle_ticks bunu ele veriyor: ~60/sn). Karşılaştırma ve
//                 teşhis için kaydediliyor, karar için değil.
//
//   screen_ms   — ASIL ÖLÇÜT. Pencerenin çıkacağı ekran pikselini masaüstü
//                 DC'sinden yoklar ve rengin değiştiği anı damgalar. Hiçbir
//                 çerçeve callback'ine güvenmez; DWM'in gerçekten sunduğu
//                 kareyi görür. Tauri tarafında da birebir aynı yöntem
//                 kullanılabilir — karşılaştırmayı adil yapan budur.
//
// Ortam değişkenleri:
//   P0_PROFILE = hide-short | hide-long | offscreen-short
//   P0_HIDE    = raw -> ham ShowWindow(SW_HIDE/SW_SHOW)
//                wpf -> WPF'in kendi Hide()/Show() yolu (altta yine SW_HIDE)

using System;
using System.Collections.Concurrent;
using System.Collections.Generic;
using System.Globalization;
using System.IO;
using System.Runtime.InteropServices;
using System.Text;
using System.Threading;
using System.Windows;
using System.Windows.Interop;
using System.Windows.Media;

namespace p0_wpf;

public partial class MainWindow : Window
{
    // --- interop ------------------------------------------------------------

    [DllImport("user32.dll")] private static extern bool ShowWindow(IntPtr hWnd, int nCmdShow);
    [DllImport("user32.dll")] private static extern bool IsWindowVisible(IntPtr hWnd);
    [DllImport("user32.dll", SetLastError = true)]
    private static extern bool SetWindowPos(IntPtr hWnd, IntPtr after, int x, int y, int cx, int cy, uint flags);
    [DllImport("user32.dll", SetLastError = true)]
    private static extern IntPtr GetWindowLongPtrW(IntPtr hWnd, int nIndex);
    [DllImport("user32.dll", SetLastError = true)]
    private static extern IntPtr SetWindowLongPtrW(IntPtr hWnd, int nIndex, IntPtr dwNewLong);
    [DllImport("user32.dll")] private static extern bool GetWindowRect(IntPtr hWnd, out RECT r);
    [DllImport("user32.dll")] private static extern IntPtr GetDC(IntPtr hWnd);
    [DllImport("user32.dll")] private static extern int ReleaseDC(IntPtr hWnd, IntPtr hDC);
    [DllImport("gdi32.dll")] private static extern uint GetPixel(IntPtr hdc, int x, int y);
    [DllImport("kernel32.dll")] private static extern bool QueryPerformanceCounter(out long v);
    [DllImport("kernel32.dll")] private static extern bool QueryPerformanceFrequency(out long v);
    [DllImport("kernel32.dll")] private static extern bool AttachConsole(int processId);

    [StructLayout(LayoutKind.Sequential)]
    private struct RECT { public int Left, Top, Right, Bottom; }

    private const int SW_HIDE = 0, SW_SHOW = 5;
    private const int GWL_EXSTYLE = -20;
    private const long WS_EX_NOACTIVATE = 0x08000000, WS_EX_TOOLWINDOW = 0x00000080;
    private const uint SWP_NOSIZE = 0x0001, SWP_NOZORDER = 0x0004, SWP_NOACTIVATE = 0x0010;
    private const uint CLR_INVALID = 0xFFFFFFFF;

    private const int OFFSCREEN_X = -20_000, OFFSCREEN_Y = -20_000;
    private const int ONSCREEN_X = 120, ONSCREEN_Y = 120;

    // --- ölçüm durumu -------------------------------------------------------

    private const int PhaseIdle = 0, PhaseWaitRaf1 = 1, PhaseWaitRaf2 = 2;

    private long _freq;
    private long _t0;
    private long _raf1;
    private int _phase = PhaseIdle;
    private int _idleTicks;
    private int _k;
    private IntPtr _hwnd;
    private int _probeX, _probeY;

    private readonly BlockingCollection<(double Raf1, double Raf2)> _results = new();

    // --- profiller ----------------------------------------------------------

    private enum Mode { Hide, Offscreen }

    /// Gizleme yolu. `Raw` Win32'yi doğrudan çağırır; `Wpf` çerçevenin kendi
    /// Hide()/Show() yolunu kullanır — o da altta ShowWindow(SW_HIDE) çağırır,
    /// yani "gerçek SW_HIDE" kısıtı iki yolda da korunur.
    private enum HideVia { Raw, Wpf }

    private static HideVia PickHideVia() =>
        (Environment.GetEnvironmentVariable("P0_HIDE") ?? "raw") == "wpf" ? HideVia.Wpf : HideVia.Raw;

    private readonly record struct Profile(string Name, Mode Mode, int Reps, int IdleMs);

    private const int Warmup = 3;
    private const int WarmupIdleMs = 2_000;

    private static Profile PickProfile()
    {
        var name = Environment.GetEnvironmentVariable("P0_PROFILE") ?? "hide-short";
        return name switch
        {
            "hide-long" => new Profile("hide-long", Mode.Hide, 5, 60_000),
            "offscreen-short" => new Profile("offscreen-short", Mode.Offscreen, 25, 2_000),
            _ => new Profile("hide-short", Mode.Hide, 25, 2_000),
        };
    }

    // --- kurulum ------------------------------------------------------------

    public MainWindow()
    {
        InitializeComponent();
        AttachConsole(-1); // PowerShell'den calistirildiginda ciktilar gorunsun
        QueryPerformanceFrequency(out _freq);

        CompositionTarget.Rendering += OnRendering;

        Loaded += (_, _) =>
        {
            _hwnd = new WindowInteropHelper(this).Handle;
            MakeNoActivate(_hwnd);
            var t = new Thread(Drive) { IsBackground = true, Name = "p0-driver" };
            t.Start();
        };
    }

    private static void MakeNoActivate(IntPtr h)
    {
        long cur = GetWindowLongPtrW(h, GWL_EXSTYLE).ToInt64();
        SetWindowLongPtrW(h, GWL_EXSTYLE, new IntPtr(cur | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW));
    }

    // --- çerçeve sinyali (teşhis) -------------------------------------------

    private void OnRendering(object? sender, EventArgs e)
    {
        QueryPerformanceCounter(out long t1);

        switch (Volatile.Read(ref _phase))
        {
            case PhaseWaitRaf1:
                Volatile.Write(ref _raf1, t1 - Volatile.Read(ref _t0));
                Volatile.Write(ref _phase, PhaseWaitRaf2);
                break;

            case PhaseWaitRaf2:
                long d2 = t1 - Volatile.Read(ref _t0);
                long d1 = Volatile.Read(ref _raf1);
                Volatile.Write(ref _phase, PhaseIdle);
                _results.Add((Ms(d1), Ms(d2)));
                break;

            default:
                Interlocked.Increment(ref _idleTicks);
                break;
        }

        // Her karede gercek bir degisiklik — yoksa kompozitor kareyi atlayabilir.
        _k++;
        Counter.Text = "kare " + _k;
        Fill.Width = (_k % 100) * 2.5;
    }

    private double Ms(long ticks) => ticks * 1000.0 / _freq;

    // --- ekran sinyali (asıl ölçüt) -----------------------------------------

    private uint ProbePixel()
    {
        IntPtr dc = GetDC(IntPtr.Zero);
        if (dc == IntPtr.Zero) return CLR_INVALID;
        uint c = GetPixel(dc, _probeX, _probeY);
        ReleaseDC(IntPtr.Zero, dc);
        return c;
    }

    /// Pencerenin gerçekten göründüğü anı yakalar: masaüstü pikselini sıkı
    /// döngüde yoklar, `baseColor`dan farklılaştığı anda damga alır.
    /// Zaman aşımında NaN döner.
    private double WaitForPixelChange(uint baseColor, int timeoutMs)
    {
        QueryPerformanceCounter(out long start);
        long limit = start + (long)(_freq * (timeoutMs / 1000.0));
        while (true)
        {
            if (ProbePixel() != baseColor)
            {
                QueryPerformanceCounter(out long t1);
                long t0 = Volatile.Read(ref _t0);
                return t0 == 0 ? double.NaN : Ms(t1 - t0);
            }
            QueryPerformanceCounter(out long now);
            if (now > limit) return double.NaN;
        }
    }

    // --- sürücü -------------------------------------------------------------

    private void Drive()
    {
        var prof = PickProfile();
        var via = PickHideVia();

        Dispatcher.Invoke(() => ShowWindow(_hwnd, SW_SHOW));
        Thread.Sleep(1500);

        // Yoklama noktasını gerçek pencere dikdörtgeninden hesapla — DPI
        // ölçeklemesi ne olursa olsun doğru piksele düşsün.
        RECT r = default;
        Dispatcher.Invoke(() => GetWindowRect(_hwnd, out r));
        _probeX = r.Left + 45;
        _probeY = r.Top + 60;

        uint shownColor = ProbePixel();
        Dispatcher.Invoke(() => ShowWindow(_hwnd, SW_HIDE));
        Thread.Sleep(500);
        uint deskColor = ProbePixel();
        Dispatcher.Invoke(() => ShowWindow(_hwnd, SW_SHOW));
        Thread.Sleep(500);

        bool probeOk = shownColor != deskColor && shownColor != CLR_INVALID && deskColor != CLR_INVALID;
        Console.WriteLine($"[p0-wpf] yoklama noktasi=({_probeX},{_probeY}) " +
                          $"pencere=0x{shownColor:X6} masaustu=0x{deskColor:X6} " +
                          $"-> {(probeOk ? "GECERLI" : "GECERSIZ (renkler ayirt edilemiyor)")}");
        if (!probeOk)
        {
            Console.Error.WriteLine("[p0-wpf] HATA: ekran yoklamasi kalibre edilemedi. " +
                                    "Pencerenin arkasindaki masaustu rengi plakaya cok benziyor olabilir.");
        }

        Console.WriteLine($"[p0-wpf] profil={prof.Name} mod={(prof.Mode == Mode.Hide ? "SW_HIDE" : "offscreen")} " +
                          $"gizleme={(via == HideVia.Raw ? "raw-win32" : "wpf-api")} " +
                          $"tekrar={prof.Reps} bekleme={prof.IdleMs}ms (+{Warmup} isinma)");

        var rows = new List<(int Iter, double Screen, double Raf1, double Raf2, int Idle, bool Vis)>();
        int total = Warmup + prof.Reps;

        for (int i = 0; i < total; i++)
        {
            bool warm = i < Warmup;
            int idle = warm ? WarmupIdleMs : prof.IdleMs;

            // 1) gizle
            Dispatcher.Invoke(() =>
            {
                if (prof.Mode == Mode.Offscreen)
                    SetWindowPos(_hwnd, IntPtr.Zero, OFFSCREEN_X, OFFSCREEN_Y, 0, 0,
                                 SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
                else if (via == HideVia.Wpf) Hide();
                else ShowWindow(_hwnd, SW_HIDE);
            });
            Thread.Sleep(120);

            // 2) gizliyken kac kare atesleniyor, pencere Win32'ye gore gizli mi?
            Volatile.Write(ref _idleTicks, 0);
            Volatile.Write(ref _t0, 0);
            while (_results.TryTake(out _)) { }
            Thread.Sleep(idle);
            int idleTicks = Volatile.Read(ref _idleTicks);
            bool visWhileIdle = IsWindowVisible(_hwnd);
            uint baseColor = ProbePixel();

            // 3) tetikle — t0 ve gosterme ayni closure'da, UI thread'inde.
            //    BeginInvoke: surucu thread'i bloke olmasin ki pikseli yoklayabilsin.
            Dispatcher.BeginInvoke(() =>
            {
                Volatile.Write(ref _phase, PhaseIdle);
                QueryPerformanceCounter(out long t);
                Volatile.Write(ref _t0, t);
                Volatile.Write(ref _phase, PhaseWaitRaf1);
                if (prof.Mode == Mode.Offscreen)
                    SetWindowPos(_hwnd, IntPtr.Zero, ONSCREEN_X, ONSCREEN_Y, 0, 0,
                                 SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
                else if (via == HideVia.Wpf) Show();
                else ShowWindow(_hwnd, SW_SHOW);
            });

            // 4) ASIL OLCUM: piksel ne zaman degisti?
            double screenMs = WaitForPixelChange(baseColor, 3000);

            // 5) cerceve sinyali (teshis)
            double r1 = double.NaN, r2 = double.NaN;
            if (_results.TryTake(out var rr, TimeSpan.FromSeconds(15))) { r1 = rr.Raf1; r2 = rr.Raf2; }
            else Volatile.Write(ref _phase, PhaseIdle);

            if (warm)
            {
                Console.WriteLine($"[p0-wpf]  isinma {i + 1}/{Warmup}  screen={screenMs:F2} raf1={r1:F2} raf2={r2:F2}");
            }
            else
            {
                int n = i - Warmup + 1;
                Console.WriteLine($"[p0-wpf]  {n}/{prof.Reps}  screen={screenMs:F2}ms " +
                                  $"raf1={r1:F2} raf2={r2:F2} idle_ticks={idleTicks} gizliyken_gorunur={visWhileIdle}");
                rows.Add((n, screenMs, r1, r2, idleTicks, visWhileIdle));
            }
        }

        WriteCsv(prof.Name, via, rows);
        Console.WriteLine($"[p0-wpf] bitti, {rows.Count} satir yazildi");
        Dispatcher.Invoke(() => Application.Current.Shutdown());
    }

    // --- çıktı --------------------------------------------------------------

    /// exe bin/Release/... altinda oldugu icin sabit ".." sayisi kirilgan;
    /// docs/ ve design/ klasorlerini barindiran proje kokunu yukari yuruyerek bul.
    private static string FindProjectRoot()
    {
        var d = new DirectoryInfo(AppContext.BaseDirectory);
        while (d != null)
        {
            if (Directory.Exists(Path.Combine(d.FullName, "docs")) &&
                Directory.Exists(Path.Combine(d.FullName, "design")))
                return d.FullName;
            d = d.Parent;
        }
        return AppContext.BaseDirectory;
    }

    private static void WriteCsv(string profileName, HideVia via,
                                 List<(int Iter, double Screen, double Raf1, double Raf2, int Idle, bool Vis)> rows)
    {
        var dir = Path.Combine(FindProjectRoot(), "measurements");
        Directory.CreateDirectory(dir);
        var suffix = via == HideVia.Wpf ? "-wpfapi" : "";
        var path = Path.Combine(dir, $"wpf-{profileName}{suffix}.csv");

        var sb = new StringBuilder("iter,screen_ms,raf1_ms,raf2_ms,idle_ticks,visible_while_idle\n");
        foreach (var r in rows)
            sb.Append(string.Format(CultureInfo.InvariantCulture,
                "{0},{1:F3},{2:F3},{3:F3},{4},{5}\n",
                r.Iter, r.Screen, r.Raf1, r.Raf2, r.Idle, r.Vis ? 1 : 0));

        File.WriteAllText(path, sb.ToString());
        Console.WriteLine($"[p0-wpf] yazildi: {path}");
    }
}
