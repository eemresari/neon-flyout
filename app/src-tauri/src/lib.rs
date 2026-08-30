//! neon-flyout — sürücü.
//!
//! Akış (`docs/ARCHITECTURE.md`):
//! `SMTC/ses olayı → Adapter thread → mpsc → Core + deadline → emit → WebView → SW_SHOW`
//!
//! Bu dosyanın tek işi bu zinciri kurmak ve core'un ürettiği efektleri
//! uygulamak. Karar core'da, Windows çağrısı adapters'ta.

mod adapters;
mod core;

use std::sync::mpsc::{channel, Receiver, RecvTimeoutError, SyncSender};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

use crate::adapters::audio::MeterCmd;
use crate::core::{DomainEvent, Effect, Engine, EngineConfig};

/// Durumun aktığı kanal: her kare tam `PlayerState` taşır.
const KANAL: &str = "flyout";
/// Tepe metresi ayrı kanalda ve core'dan geçmiyor — saniyede 30 örnek, tetik
/// değil, durumun parçası değil. `KANAL`'a konsa her örnek kapak base64'ünü de
/// yeniden gönderirdi. Yalnızca kutu ekrandayken akar.
const KANAL_TEPE: &str = "peak";

#[derive(Clone, Copy, Serialize)]
struct Tepe {
    peak: f32,
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let (tx, rx) = channel::<DomainEvent>();

    tauri::Builder::default()
        .setup(move |app| {
            let win = app.get_webview_window("main").expect("main penceresi yok");
            // Pencere açılışta bir kez kurulur ve bir daha yok edilmez; gösterme
            // ve gizleme yalnızca ShowWindow (gecikme bütçesi bunu gerektiriyor).
            let hwnd = win.hwnd()?.0 as isize;
            adapters::window::apply_styles(hwnd);
            adapters::window::place(hwnd);
            // Gizliyken WebView2'nin render dongusunu durdurur; `SW_HIDE` tek
            // basina durdurmuyor (`docs/P3-SONUC.md`).
            adapters::webview::capture(&win);

            // Windows'un kendi kutusu: bulunursa bastırılır, bulunmazsa sessizce
            // geçilir ve uygulama normal çalışır (`docs/P4-ARASTIRMA.md`).
            // Geri alma `RunEvent::Exit`'te; süreç öldürülürse çalışmaz.
            if adapters::native_flyout::suppress() {
                eprintln!("[native_flyout] Windows'un flyout'u bastirildi");
            }

            adapters::smtc::spawn(tx.clone())?;
            adapters::audio::spawn(tx.clone())?;

            let sink = app.handle().clone();
            let meter = adapters::audio::spawn_meter(move |peak| {
                let _ = sink.emit(KANAL_TEPE, Tepe { peak });
            })?;

            let handle = app.handle().clone();
            std::thread::Builder::new()
                .name("core".into())
                .spawn(move || core_loop(handle, hwnd, meter, rx))?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("Tauri baslatilamadi")
        .run(|_app, event| {
            // Bastırma geri alınabilir olmalı: kullanıcının masaüstünde kalıcı
            // bir değişiklik bırakmıyoruz. Bastırma yapılmadıysa no-op.
            if let tauri::RunEvent::Exit = event {
                adapters::native_flyout::restore();
            }
        });
}

/// Tek yazıcı görev. Ayrı bir zamanlayıcı thread'i yok: deadline'a kalan süre
/// `recv_timeout`'un beklemesi olarak geçiyor, böylece üst üste binen
/// zamanlayıcı ihtimali ortadan kalkıyor.
fn core_loop(app: AppHandle, hwnd: isize, meter: SyncSender<MeterCmd>, rx: Receiver<DomainEvent>) {
    let start = Instant::now();
    let now = move || start.elapsed().as_millis() as u64;
    let mut engine = Engine::new(EngineConfig::default());

    loop {
        let effects = match engine.next_timeout(now()) {
            Some(ms) => match rx.recv_timeout(Duration::from_millis(ms)) {
                Ok(ev) => engine.handle(ev, now()),
                Err(RecvTimeoutError::Timeout) => engine.tick(now()),
                Err(RecvTimeoutError::Disconnected) => break,
            },
            // Kutu gizli: uyanmak için sebep yok, olay bekle.
            None => match rx.recv() {
                Ok(ev) => engine.handle(ev, now()),
                Err(_) => break,
            },
        };

        for effect in effects {
            apply(&app, hwnd, &meter, effect);
        }
    }
}

fn apply(app: &AppHandle, hwnd: isize, meter: &SyncSender<MeterCmd>, effect: Effect) {
    match effect {
        // İçerik pencereden ÖNCE gider: ilk boyanan kare doğru içeriği taşısın.
        Effect::Emit(frame) => {
            let _ = app.emit(KANAL, frame);
        }
        Effect::Show => {
            let _ = app.run_on_main_thread(move || {
                // Once WebView uyanir, sonra pencere gorunur: ters sirada ilk
                // kare bos cikabilir.
                adapters::webview::set_visible(true);
                adapters::window::show(hwnd);
            });
            // Metre kutunun ömrüyle birebir: gizliyken tek örnek bile alınmıyor.
            let _ = meter.try_send(MeterCmd::Start);
        }
        Effect::Hide => {
            let _ = app.run_on_main_thread(move || {
                // Once pencere gizlenir: WebView'i gorunmez yapmak bos bir kare
                // bastirabilir, o kare ekranda gorunmesin.
                adapters::window::hide(hwnd);
                adapters::webview::set_visible(false);
            });
            let _ = meter.try_send(MeterCmd::Stop);
        }
    }
}
