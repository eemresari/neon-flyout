//! WebView2 controller — render döngüsünü kutu gizliyken durdurur.
//!
//! P0'ın yan bulgusu: `SW_HIDE` pencereyi Win32/DWM'den gizliyor ama WebView2
//! saniyede 185 kare üretmeye devam ediyordu (60 sn'de 11 098 kare). Kutu günün
//! neredeyse tamamını gizli geçirdiği için bu, boşuna dönen bir GPU demekti.
//!
//! Çözüm `ICoreWebView2Controller::SetIsVisible(false)`. P3'te ölçüldü
//! (`docs/P3-SONUC.md`): gizliyken üretilen kare **0**, uyanma bedeli
//! `screen_ms` p95 **11.6 ms** — ham `SW_HIDE`'ın 12.1 ms'inden bile düşük.
//! Yani bedava.
//!
//! **`TrySuspend` bilerek kullanılmadı.** Gecikmesi aynı (11.8 ms) ama JavaScript'i
//! de donduruyor; bizim akışta içerik pencereden ÖNCE gidiyor (`Effect::Emit` →
//! `Effect::Show`), donmuş bir WebView o olayı ancak `Resume`'dan sonra işler ve
//! ilk boyanan kare bayat içerik taşır. Kazancı bellek, gecikme değil — gerekirse
//! P5'te ayrıca değerlendirilir.

use std::sync::atomic::{AtomicUsize, Ordering};

use webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Controller;
use windows::core::Interface;

/// Controller'ın ham işaretçisi. COM nesnesinin thread benzeşimi var:
/// **yalnızca UI thread'inden** çağrılır (`lib.rs` → `run_on_main_thread`).
static CTRL: AtomicUsize = AtomicUsize::new(0);

/// Kurulumda bir kez. `with_webview` kapanışı UI thread'ine kuyruklanır, bu
/// yüzden anında hazır olmayabilir — `set_visible` o aralıkta sessizce hiçbir
/// şey yapmaz ve WebView açılıştaki görünür halinde kalır. Zararsız: ilk
/// gizlemeye kadar geçen birkaç yüz ms.
pub fn capture(win: &tauri::WebviewWindow) {
    let _ = win.with_webview(|pw| {
        let ctrl = pw.controller();
        let raw = ctrl.as_raw();
        // Bir AddRef bilerek sızdırılıyor: controller uygulama ömrü boyunca
        // yaşıyor, karşılığında işaretçinin geçerliliği garanti oluyor.
        std::mem::forget(ctrl);
        CTRL.store(raw as usize, Ordering::Release);

        // Pencere `visible: false` doğuyor; WebView de öyle başlasın, yoksa ilk
        // gösterime kadar boşuna render eder.
        set_visible(false);
    });
}

/// `true` göstermeden ÖNCE, `false` gizledikten SONRA çağrılır — sıra `lib.rs`'te.
pub fn set_visible(visible: bool) {
    let raw = CTRL.load(Ordering::Acquire) as *mut std::ffi::c_void;
    if let Some(ctrl) = unsafe { ICoreWebView2Controller::from_raw_borrowed(&raw) } {
        let _ = unsafe { ctrl.SetIsVisible(visible) };
    }
}
