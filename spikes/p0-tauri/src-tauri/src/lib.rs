//! P0 gecikme kapısı — Tauri tarafı.
//!
//! Bu ATILACAK koddur (ROADMAP P0). Güzelliğe değil, ölçümün doğruluğuna optimize
//! edildi. Mimarinin "Win32 sadece adapters katmanında" kuralı P0'ı bağlamaz.
//!
//! Ölçtüğü şey: gösterme çağrısından hemen önce alınan QPC damgası ile, gizliyken
//! duran requestAnimationFrame döngüsünün uyanıp ateşlediği ilk (`raf1`) ve ikinci
//! (`raf2`) callback arasındaki fark.
//!
//!   raf1 -> boyamadan ÖNCE çalışır, gerçeği olduğundan DÜŞÜK gösterir
//!   raf2 -> önceki karenin teslim edildiğini ima eder, YÜKSEK gösterir
//!   gerçek "ilk piksel" ikisinin arasındadır; kapı kararı raf2'ye göre verilir
//!
//! `idle_ticks` sağlık kontrolüdür: gizli bekleme boyunca kaç rAF ateşlendiğini
//! sayar. **~0 değilse ölçüm bir uyanma ölçümü değildir** — WebView hiç uyumamıştır.
//! `visible_while_idle` ise Win32'nin pencereyi gerçekten gizli sayıp saymadığını
//! söyler; ikisi birlikte "kim uyumuyor?" sorusunu ayırt eder.
//!
//! Ortam değişkenleri:
//!   P0_PROFILE = hide-short | hide-long | offscreen-short
//!   P0_HIDE    = raw        -> ham ShowWindow(SW_HIDE/SW_SHOW)
//!                tauri      -> Tauri'nin kendi window.hide()/show() yolu
//!                suspend    -> raw + WebView2 controller `SetIsVisible(false)`
//!                trysuspend -> suspend + `ICoreWebView2_3::TrySuspend()`
//!
//! Son iki yol P0'ın yan bulgusu için eklendi (P3): ham `SW_HIDE` WebView2'nin
//! render döngüsünü durdurmuyordu (gizliyken 185 kare/sn). `idle_ticks` bu iki
//! yolda ~0'a düşmeli; `screen_ms` ise uyanma bedelini ilk kez **gerçekten**
//! ölçüyor, çünkü artık uyutulacak bir şey var.

use std::sync::atomic::{AtomicI64, AtomicU32, AtomicU8, AtomicUsize, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, SyncSender};
use std::sync::Mutex;
use std::time::Duration;

use tauri::{AppHandle, Manager};

use webview2_com::Microsoft::Web::WebView2::Win32::{ICoreWebView2Controller, ICoreWebView2_3};
use webview2_com::TrySuspendCompletedHandler;
use windows::core::Interface;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{GetDC, GetPixel, ReleaseDC};
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows::Win32::UI::WindowsAndMessaging::{
    GetWindowLongPtrW, GetWindowRect, IsWindowVisible, SetWindowLongPtrW, SetWindowPos, ShowWindow,
    GWL_EXSTYLE, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SW_HIDE, SW_SHOW, WS_EX_NOACTIVATE,
    WS_EX_TOOLWINDOW,
};

// --- ölçüm durumu -----------------------------------------------------------

const PHASE_IDLE: u8 = 0;
const PHASE_WAIT_RAF1: u8 = 1;
const PHASE_WAIT_RAF2: u8 = 2;

struct P0 {
    freq: i64,
    t0: AtomicI64,
    phase: AtomicU8,
    raf1: AtomicI64,
    idle_ticks: AtomicU32,
    result_tx: Mutex<Option<SyncSender<(f64, f64)>>>,
    ready_tx: Mutex<Option<SyncSender<()>>>,
}

fn qpc() -> i64 {
    let mut v = 0i64;
    unsafe { QueryPerformanceCounter(&mut v).ok() };
    v
}

fn qpf() -> i64 {
    let mut v = 0i64;
    unsafe { QueryPerformanceFrequency(&mut v).ok() };
    v
}

// --- WebView'den gelen sinyaller --------------------------------------------

#[tauri::command]
fn frame(state: tauri::State<'_, P0>) {
    let t1 = qpc();
    match state.phase.load(Ordering::Acquire) {
        PHASE_WAIT_RAF1 => {
            let d = t1 - state.t0.load(Ordering::Acquire);
            state.raf1.store(d, Ordering::Release);
            state.phase.store(PHASE_WAIT_RAF2, Ordering::Release);
        }
        PHASE_WAIT_RAF2 => {
            let d2 = t1 - state.t0.load(Ordering::Acquire);
            let d1 = state.raf1.load(Ordering::Acquire);
            state.phase.store(PHASE_IDLE, Ordering::Release);
            let f = state.freq as f64;
            let ms = |t: i64| t as f64 * 1000.0 / f;
            if let Some(tx) = state.result_tx.lock().unwrap().as_ref() {
                let _ = tx.try_send((ms(d1), ms(d2)));
            }
        }
        _ => {
            state.idle_ticks.fetch_add(1, Ordering::Relaxed);
        }
    }
}

#[tauri::command]
fn ready(state: tauri::State<'_, P0>) {
    if let Some(tx) = state.ready_tx.lock().unwrap().take() {
        let _ = tx.try_send(());
    }
}

// --- pencere işlemleri ------------------------------------------------------

fn h(raw: isize) -> HWND {
    HWND(raw as *mut core::ffi::c_void)
}

/// Odak çalmayı ve görev çubuğu girişini engelle. Bunlar olmadan 25 tekrar
/// boyunca ön plandaki pencere sürekli değişir ve ölçüm kirlenir.
fn make_noactivate(raw: isize) {
    unsafe {
        let cur = GetWindowLongPtrW(h(raw), GWL_EXSTYLE);
        let want = cur | (WS_EX_NOACTIVATE.0 as isize) | (WS_EX_TOOLWINDOW.0 as isize);
        SetWindowLongPtrW(h(raw), GWL_EXSTYLE, want);
    }
}

const OFFSCREEN_X: i32 = -20_000;
const OFFSCREEN_Y: i32 = -20_000;
const ONSCREEN_X: i32 = 120;
const ONSCREEN_Y: i32 = 120;

fn move_to(raw: isize, x: i32, y: i32) {
    unsafe {
        let _ = SetWindowPos(
            h(raw),
            None,
            x,
            y,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
    }
}

fn is_visible(raw: isize) -> bool {
    unsafe { IsWindowVisible(h(raw)).as_bool() }
}

// --- WebView2 controller ----------------------------------------------------
//
// `PlatformWebview::controller()` yalnızca `with_webview` kapanışının içinde
// elde edilebiliyor ve o kapanış UI thread'inde çalışıyor. Ölçüm sürücüsü ise
// ayrı bir thread; bu yüzden işaretçiyi bir kez yakalayıp saklıyoruz. COM
// nesnesinin thread benzeşimi var, dolayısıyla **çağrılar yalnızca
// `run_on_main_thread` içinden** yapılır.

static CTRL: AtomicUsize = AtomicUsize::new(0);
static WV3: AtomicUsize = AtomicUsize::new(0);

/// Yakalarken bilerek bir AddRef sızdırıyoruz: spike süreç sonunda ölüyor,
/// karşılığında işaretçinin ölçüm boyunca geçerli kalacağı garanti oluyor.
fn capture_webview(win: &tauri::WebviewWindow) {
    let _ = win.with_webview(|pw| {
        let ctrl = pw.controller();
        let raw = ctrl.as_raw();
        std::mem::forget(ctrl);
        CTRL.store(raw as usize, Ordering::Release);

        // TrySuspend/Resume `ICoreWebView2_3` üzerinde; controller'da değil.
        let wv3 = unsafe { ICoreWebView2Controller::from_raw_borrowed(&raw) }
            .and_then(|c| unsafe { c.CoreWebView2() }.ok())
            .and_then(|wv| wv.cast::<ICoreWebView2_3>().ok());
        if let Some(wv3) = wv3 {
            let r = wv3.as_raw();
            std::mem::forget(wv3);
            WV3.store(r as usize, Ordering::Release);
        }
    });
}

fn with_ctrl<R>(f: impl FnOnce(&ICoreWebView2Controller) -> R) -> Option<R> {
    let raw = CTRL.load(Ordering::Acquire) as *mut core::ffi::c_void;
    unsafe { ICoreWebView2Controller::from_raw_borrowed(&raw) }.map(f)
}

fn with_wv3<R>(f: impl FnOnce(&ICoreWebView2_3) -> R) -> Option<R> {
    let raw = WV3.load(Ordering::Acquire) as *mut core::ffi::c_void;
    unsafe { ICoreWebView2_3::from_raw_borrowed(&raw) }.map(f)
}

fn webview_set_visible(v: bool) {
    with_ctrl(|c| unsafe { c.SetIsVisible(v) });
}

/// Asenkron ve `IsVisible == false` şart. Sonucu yalnızca teşhis için basıyoruz;
/// askıya alınma bekleme süresi (en az 2 sn) içinde tamamlanıyor.
fn webview_try_suspend() {
    with_wv3(|wv| unsafe {
        let handler = TrySuspendCompletedHandler::create(Box::new(|hr, ok| {
            if hr.is_err() || !ok {
                eprintln!("[p0] TrySuspend basarisiz: hr={hr:?} ok={ok}");
            }
            Ok(())
        }));
        let _ = wv.TrySuspend(&handler);
    });
}

/// Senkron. `TrySuspend` tamamlanmamışsa da güvenli (no-op).
fn webview_resume() {
    with_wv3(|wv| unsafe { wv.Resume() });
}

// --- ekran sinyali (asıl ölçüt) ---------------------------------------------
//
// Çerçeveye ait hiçbir callback "pencere gerçekten boyandı mı?" sorusuna cevap
// vermiyor: rAF da, WPF'in CompositionTarget.Rendering'i de pencere gizliyken
// dönmeye devam ediyor. Bu yüzden asıl ölçüt dışarıdan bakıyor — masaüstü
// DC'sinden tek bir pikseli yokluyor ve rengin değiştiği anı damgalıyor.
// Aynı yöntem WPF spike'ında da kullanılıyor; karşılaştırmayı adil yapan bu.

const CLR_INVALID: u32 = 0xFFFF_FFFF;

fn probe_pixel(x: i32, y: i32) -> u32 {
    unsafe {
        let dc = GetDC(None);
        if dc.is_invalid() {
            return CLR_INVALID;
        }
        let c = GetPixel(dc, x, y).0;
        ReleaseDC(None, dc);
        c
    }
}

fn window_rect(raw: isize) -> RECT {
    let mut r = RECT::default();
    unsafe {
        let _ = GetWindowRect(h(raw), &mut r);
    }
    r
}

/// `base` renginden farklı bir piksel görülene kadar sıkı döngüde yoklar ve
/// t0'a göre geçen süreyi ms olarak döndürür. Zaman aşımında NaN.
fn wait_for_pixel_change(x: i32, y: i32, base: u32, timeout_ms: u64, t0: &AtomicI64, freq: i64) -> f64 {
    let start = qpc();
    let limit = start + (freq as f64 * (timeout_ms as f64 / 1000.0)) as i64;
    loop {
        if probe_pixel(x, y) != base {
            let t1 = qpc();
            let t0v = t0.load(Ordering::Acquire);
            return if t0v == 0 {
                f64::NAN
            } else {
                (t1 - t0v) as f64 * 1000.0 / freq as f64
            };
        }
        if qpc() > limit {
            return f64::NAN;
        }
    }
}

// --- koşu profilleri --------------------------------------------------------

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Hide,
    Offscreen,
}

/// Gizleme yolu. `Raw` Win32'yi doğrudan çağırır; `Tauri` çerçevenin kendi
/// yolunu kullanır — bu yol WebView2 controller'ının `IsVisible` bayrağına da
/// dokunabilir, ham ShowWindow dokunmaz. Fark ölçümün geçerliliğini belirliyor.
#[derive(Clone, Copy, PartialEq)]
enum HideVia {
    Raw,
    Tauri,
    /// `SW_HIDE` + controller `SetIsVisible(false)`.
    Suspend,
    /// Üsttekine ek olarak `TrySuspend()` — süreç belleği de bırakılır.
    TrySuspend,
}

impl HideVia {
    /// WebView2'ye dokunan yollar. `Raw`/`Tauri` yalnızca Win32 penceresini oynatır.
    fn touches_webview(self) -> bool {
        matches!(self, HideVia::Suspend | HideVia::TrySuspend)
    }

    fn csv_suffix(self) -> &'static str {
        match self {
            HideVia::Raw => "",
            HideVia::Tauri => "-tauriapi",
            HideVia::Suspend => "-suspend",
            HideVia::TrySuspend => "-trysuspend",
        }
    }

    fn label(self) -> &'static str {
        match self {
            HideVia::Raw => "raw-win32",
            HideVia::Tauri => "tauri-api",
            HideVia::Suspend => "raw+SetIsVisible(false)",
            HideVia::TrySuspend => "raw+SetIsVisible(false)+TrySuspend",
        }
    }
}

struct Profile {
    name: &'static str,
    mode: Mode,
    reps: usize,
    idle_ms: u64,
}

fn profile(name: &str) -> Profile {
    match name {
        "hide-long" => Profile { name: "hide-long", mode: Mode::Hide, reps: 5, idle_ms: 60_000 },
        "offscreen-short" => {
            Profile { name: "offscreen-short", mode: Mode::Offscreen, reps: 25, idle_ms: 2_000 }
        }
        _ => Profile { name: "hide-short", mode: Mode::Hide, reps: 25, idle_ms: 2_000 },
    }
}

/// Isınma tekrarları CSV'ye yazılmaz ve her profilde kısa bekleme kullanır —
/// ısınma JIT/ilk-yükleme ile ilgilidir, bekleme derinliğiyle değil.
const WARMUP: usize = 3;
const WARMUP_IDLE_MS: u64 = 2_000;

// --- sürücü -----------------------------------------------------------------

struct Row {
    iter: usize,
    screen: f64,
    raf1: f64,
    raf2: f64,
    idle_ticks: u32,
    visible_while_idle: bool,
}

fn drive(
    app: AppHandle,
    raw: isize,
    prof: Profile,
    via: HideVia,
    ready_rx: Receiver<()>,
    rx: Receiver<(f64, f64)>,
) {
    // WebView2 gizli bir pencerede belgeyi yükler ama hiç boyamaz. Ölçümün ilk
    // tekrarı "ilk yükleme" olmasın diye pencereyi bir kez gösterip gizliyoruz.
    let a = app.clone();
    let _ = a.run_on_main_thread(move || {
        make_noactivate(raw);
        unsafe {
            let _ = ShowWindow(h(raw), SW_SHOW);
        }
    });

    if ready_rx.recv_timeout(Duration::from_secs(20)).is_err() {
        eprintln!("[p0] HATA: WebView 20 sn icinde 'ready' gondermedi, cikiliyor");
        app.exit(1);
        return;
    }

    // Controller yakalanamadıysa "suspend" koşusu sessizce ham SW_HIDE'a düşer
    // ve ölçüm yanlış etiketle CSV'ye yazılır. Ölçmemek, yanlış ölçmekten iyi.
    if via.touches_webview() {
        let ctrl_ok = CTRL.load(Ordering::Acquire) != 0;
        let wv3_ok = WV3.load(Ordering::Acquire) != 0;
        println!("[p0] controller={ctrl_ok} ICoreWebView2_3={wv3_ok}");
        if !ctrl_ok || (via == HideVia::TrySuspend && !wv3_ok) {
            eprintln!("[p0] HATA: WebView2 arayuzleri yakalanamadi, cikiliyor");
            app.exit(1);
            return;
        }
    }
    std::thread::sleep(Duration::from_millis(1500));

    // Yoklama noktasını gerçek pencere dikdörtgeninden hesapla — DPI ölçeklemesi
    // ne olursa olsun doğru piksele düşsün. Sonra pencere/masaüstü renklerinin
    // ayırt edilebildiğini doğrula; edilemiyorsa ölçüt anlamsız olur.
    let r = window_rect(raw);
    let (px, py) = (r.left + 45, r.top + 60);
    let shown_color = probe_pixel(px, py);
    let a = app.clone();
    let _ = a.run_on_main_thread(move || unsafe {
        let _ = ShowWindow(h(raw), SW_HIDE);
    });
    std::thread::sleep(Duration::from_millis(500));
    let desk_color = probe_pixel(px, py);
    let a = app.clone();
    let _ = a.run_on_main_thread(move || unsafe {
        let _ = ShowWindow(h(raw), SW_SHOW);
    });
    std::thread::sleep(Duration::from_millis(500));

    let probe_ok =
        shown_color != desk_color && shown_color != CLR_INVALID && desk_color != CLR_INVALID;
    println!(
        "[p0] yoklama noktasi=({px},{py}) pencere=0x{shown_color:06X} masaustu=0x{desk_color:06X} -> {}",
        if probe_ok { "GECERLI" } else { "GECERSIZ (renkler ayirt edilemiyor)" }
    );
    if !probe_ok {
        eprintln!(
            "[p0] HATA: ekran yoklamasi kalibre edilemedi; screen_ms sutunu guvenilmez."
        );
    }

    let freq = app.state::<P0>().freq;

    println!(
        "[p0] profil={} mod={} gizleme={} tekrar={} bekleme={}ms (+{} isinma)",
        prof.name,
        if prof.mode == Mode::Hide { "SW_HIDE" } else { "offscreen" },
        via.label(),
        prof.reps,
        prof.idle_ms,
        WARMUP
    );

    let mut rows: Vec<Row> = Vec::new();
    let total = WARMUP + prof.reps;

    for i in 0..total {
        let warm = i < WARMUP;
        let idle = if warm { WARMUP_IDLE_MS } else { prof.idle_ms };

        // 1) gizle
        let a = app.clone();
        let inner = app.clone();
        let mode = prof.mode;
        let _ = a.run_on_main_thread(move || unsafe {
            match (mode, via) {
                (Mode::Offscreen, _) => move_to(raw, OFFSCREEN_X, OFFSCREEN_Y),
                (Mode::Hide, HideVia::Tauri) => {
                    if let Some(w) = inner.get_webview_window("main") {
                        let _ = w.hide();
                    }
                }
                (Mode::Hide, _) => {
                    // Önce Win32 penceresi: WebView'i görünmez yapmak boş bir
                    // kare bastırabilir, o kare ekranda görünmesin.
                    let _ = ShowWindow(h(raw), SW_HIDE);
                    if via.touches_webview() {
                        webview_set_visible(false);
                        if via == HideVia::TrySuspend {
                            // `IsVisible == false` olduktan SONRA; aksi halde reddediliyor.
                            webview_try_suspend();
                        }
                    }
                }
            }
        });
        std::thread::sleep(Duration::from_millis(120));

        // 2) gizliyken kaç kare ateşleniyor, pencere Win32'ye göre gizli mi?
        let st = app.state::<P0>();
        st.idle_ticks.store(0, Ordering::Release);
        st.t0.store(0, Ordering::Release);
        while rx.try_recv().is_ok() {} // önceki tekrardan sarkan sonuç varsa temizle
        std::thread::sleep(Duration::from_millis(idle));
        let idle_ticks = st.idle_ticks.load(Ordering::Acquire);
        let visible_while_idle = is_visible(raw);
        let base_color = probe_pixel(px, py);

        // 3) tetikle — t0 ve gösterme aynı closure'da, pencerenin sahibi thread'de
        let a = app.clone();
        let inner = app.clone();
        let mode = prof.mode;
        let _ = a.run_on_main_thread(move || {
            let s = inner.state::<P0>();
            // Pencere aramasını t0'dan ÖNCE yap; arama maliyeti ölçüme girmesin.
            let w = if via == HideVia::Tauri && mode == Mode::Hide {
                inner.get_webview_window("main")
            } else {
                None
            };
            s.phase.store(PHASE_IDLE, Ordering::Release);
            let t = qpc();
            s.t0.store(t, Ordering::Release);
            s.phase.store(PHASE_WAIT_RAF1, Ordering::Release);
            unsafe {
                match (mode, via) {
                    (Mode::Offscreen, _) => move_to(raw, ONSCREEN_X, ONSCREEN_Y),
                    (Mode::Hide, HideVia::Tauri) => {
                        if let Some(w) = w {
                            let _ = w.show();
                        }
                    }
                    (Mode::Hide, _) => {
                        // Uyandırma bedeli ölçümün İÇİNDE: t0 yukarıda alındı.
                        if via.touches_webview() {
                            if via == HideVia::TrySuspend {
                                webview_resume();
                            }
                            webview_set_visible(true);
                        }
                        let _ = ShowWindow(h(raw), SW_SHOW);
                    }
                }
            }
        });

        // 4) ASIL OLCUM: piksel ne zaman degisti?
        let screen = {
            let st = app.state::<P0>();
            wait_for_pixel_change(px, py, base_color, 3000, &st.t0, freq)
        };

        // 5) cerceve sinyali (teshis)
        let (r1, r2) = match rx.recv_timeout(Duration::from_secs(15)) {
            Ok(v) => v,
            Err(_) => {
                eprintln!("[p0] UYARI: tekrar {} icin cerceve karesi gelmedi", i + 1);
                app.state::<P0>().phase.store(PHASE_IDLE, Ordering::Release);
                (f64::NAN, f64::NAN)
            }
        };

        if warm {
            println!(
                "[p0]  isinma {}/{}  screen={:.2} raf1={:.2} raf2={:.2}",
                i + 1,
                WARMUP,
                screen,
                r1,
                r2
            );
        } else {
            let n = i - WARMUP + 1;
            println!(
                "[p0]  {}/{}  screen={:.2}ms raf1={:.2} raf2={:.2} idle_ticks={} gizliyken_gorunur={}",
                n, prof.reps, screen, r1, r2, idle_ticks, visible_while_idle
            );
            rows.push(Row {
                iter: n,
                screen,
                raf1: r1,
                raf2: r2,
                idle_ticks,
                visible_while_idle,
            });
        }
    }

    write_csv(prof.name, via, &rows);
    println!("[p0] bitti, {} satir yazildi", rows.len());
    app.exit(0);
}

fn write_csv(profile_name: &str, via: HideVia, rows: &[Row]) {
    // CARGO_MANIFEST_DIR = <proje>/spikes/p0-tauri/src-tauri
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("..")
        .join("measurements");
    if let Err(e) = std::fs::create_dir_all(&dir) {
        eprintln!("[p0] measurements klasoru acilamadi: {e}");
        return;
    }
    let suffix = via.csv_suffix();
    let path = dir.join(format!("tauri-{profile_name}{suffix}.csv"));
    let mut out = String::from("iter,screen_ms,raf1_ms,raf2_ms,idle_ticks,visible_while_idle\n");
    for r in rows {
        out.push_str(&format!(
            "{},{:.3},{:.3},{:.3},{},{}\n",
            r.iter,
            r.screen,
            r.raf1,
            r.raf2,
            r.idle_ticks,
            if r.visible_while_idle { 1 } else { 0 }
        ));
    }
    match std::fs::write(&path, out) {
        Ok(_) => println!("[p0] yazildi: {}", path.display()),
        Err(e) => eprintln!("[p0] CSV yazilamadi: {e}"),
    }
}

// --- giriş ------------------------------------------------------------------

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let prof = profile(&std::env::var("P0_PROFILE").unwrap_or_else(|_| "hide-short".into()));
    let via = match std::env::var("P0_HIDE").unwrap_or_else(|_| "raw".into()).as_str() {
        "tauri" => HideVia::Tauri,
        "suspend" => HideVia::Suspend,
        "trysuspend" => HideVia::TrySuspend,
        _ => HideVia::Raw,
    };

    let (result_tx, result_rx) = sync_channel::<(f64, f64)>(8);
    let (ready_tx, ready_rx) = sync_channel::<()>(1);

    let state = P0 {
        freq: qpf(),
        t0: AtomicI64::new(0),
        phase: AtomicU8::new(PHASE_IDLE),
        raf1: AtomicI64::new(0),
        idle_ticks: AtomicU32::new(0),
        result_tx: Mutex::new(Some(result_tx)),
        ready_tx: Mutex::new(Some(ready_tx)),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![frame, ready])
        .setup(move |app| {
            let win = app.get_webview_window("main").expect("main penceresi yok");
            let raw = win.hwnd()?.0 as isize;
            if via.touches_webview() {
                // Kuyruğa giriyor, UI thread'inde çalışıyor. `drive` zaten
                // WebView'in `ready` sinyalini bekliyor; o gelene kadar bu
                // kapanış çoktan koşmuş olur.
                capture_webview(&win);
            }
            let handle = app.handle().clone();
            std::thread::spawn(move || drive(handle, raw, prof, via, ready_rx, result_rx));
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
