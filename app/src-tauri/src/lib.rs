//! neon-flyout — sürücü.
//!
//! Akış (`docs/ARCHITECTURE.md`):
//! `SMTC/ses olayı → Adapter thread → mpsc → Core + deadline → emit → WebView → SW_SHOW`
//!
//! Bu dosyanın tek işi bu zinciri kurmak ve core'un ürettiği efektleri
//! uygulamak. Karar core'da, Windows çağrısı adapters'ta.

mod adapters;
mod core;

use std::sync::mpsc::{channel, Receiver, RecvTimeoutError};
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager};

use crate::core::{DomainEvent, Effect, Engine, EngineConfig};

/// WebView'in dinlediği tek kanal.
const KANAL: &str = "flyout";

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
            adapters::window::place_top_right(hwnd);

            adapters::smtc::spawn(tx.clone())?;
            adapters::audio::spawn(tx.clone())?;

            let handle = app.handle().clone();
            std::thread::Builder::new()
                .name("core".into())
                .spawn(move || core_loop(handle, hwnd, rx))?;
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("Tauri baslatilamadi");
}

/// Tek yazıcı görev. Ayrı bir zamanlayıcı thread'i yok: deadline'a kalan süre
/// `recv_timeout`'un beklemesi olarak geçiyor, böylece üst üste binen
/// zamanlayıcı ihtimali ortadan kalkıyor.
fn core_loop(app: AppHandle, hwnd: isize, rx: Receiver<DomainEvent>) {
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
            apply(&app, hwnd, effect);
        }
    }
}

fn apply(app: &AppHandle, hwnd: isize, effect: Effect) {
    match effect {
        // İçerik pencereden ÖNCE gider: ilk boyanan kare doğru içeriği taşısın.
        Effect::Emit(frame) => {
            let _ = app.emit(KANAL, frame);
        }
        Effect::Show => {
            let _ = app.run_on_main_thread(move || adapters::window::show(hwnd));
        }
        Effect::Hide => {
            let _ = app.run_on_main_thread(move || adapters::window::hide(hwnd));
        }
    }
}
