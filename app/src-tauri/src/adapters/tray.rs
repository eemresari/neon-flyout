//! Tepsi ikonu ve menüsü. Uygulamanın **tek** kullanıcı arayüzü giriş noktası —
//! kutunun kendisi tıklanabilir değil (`WS_EX_TRANSPARENT`).
//!
//! Menü değişiminde sıra: (1) yetkili Config güncellenir, (2) ilgili tüketiciye
//! itilir, (3) dosyaya yazılır. Dosyaya yazma başarısız olsa bile değişiklik o
//! oturumda geçerli olur.
//!
//! **Radyo düğmesi yok:** Tauri v2 menüsünde radio öğesi bulunmuyor; tema ve köşe
//! onay işaretli öğelerle gösteriliyor, karşılıklı dışlama elle yapılıyor.

use std::sync::mpsc::Sender;

use tauri::menu::{CheckMenuItem, IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager, Wry};

use crate::core::{Config, Corner, DomainEvent, Theme};
use crate::{SharedConfig, UiConfig, KANAL_AYAR};

use super::{autostart, config_file, window};

/// Menü kimliği ↔ değer ↔ etiket. Tek yerde durur; hem menü kurulumu hem olay
/// çözümü buradan okur, ikisi ayrışamaz.
const TEMALAR: [(&str, Theme, &str); 2] = [
    ("theme:cyber", Theme::Cyber, "cyber"),
    ("theme:neon", Theme::Neon, "neon"),
];

const KOSELER: [(&str, Corner, &str); 4] = [
    ("corner:top-right", Corner::TopRight, "sağ üst"),
    ("corner:top-left", Corner::TopLeft, "sol üst"),
    ("corner:bottom-right", Corner::BottomRight, "sağ alt"),
    ("corner:bottom-left", Corner::BottomLeft, "sol alt"),
];

pub fn init(app: &AppHandle, tx: Sender<DomainEvent>) -> tauri::Result<()> {
    let cfg = *app
        .state::<SharedConfig>()
        .0
        .lock()
        .expect("config mutex zehirli");

    let temalar: Vec<CheckMenuItem<Wry>> = TEMALAR
        .iter()
        .map(|(id, t, ad)| CheckMenuItem::with_id(app, *id, ad, true, cfg.theme == *t, None::<&str>))
        .collect::<tauri::Result<_>>()?;
    let koseler: Vec<CheckMenuItem<Wry>> = KOSELER
        .iter()
        .map(|(id, c, ad)| {
            CheckMenuItem::with_id(app, *id, ad, true, cfg.corner == *c, None::<&str>)
        })
        .collect::<tauri::Result<_>>()?;

    let tema_ref: Vec<&dyn IsMenuItem<Wry>> =
        temalar.iter().map(|i| i as &dyn IsMenuItem<Wry>).collect();
    let kose_ref: Vec<&dyn IsMenuItem<Wry>> =
        koseler.iter().map(|i| i as &dyn IsMenuItem<Wry>).collect();

    let tema_alt = Submenu::with_items(app, "Tema", true, &tema_ref)?;
    let kose_alt = Submenu::with_items(app, "Köşe", true, &kose_ref)?;

    let baslat = CheckMenuItem::with_id(
        app,
        "autostart",
        "Windows ile başlat",
        true,
        autostart::enabled(),
        None::<&str>,
    )?;
    let ac = MenuItem::with_id(app, "open", "Ayar dosyasını aç", true, None::<&str>)?;
    let yenile = MenuItem::with_id(app, "reload", "Ayarları yeniden yükle", true, None::<&str>)?;
    let cik = MenuItem::with_id(app, "quit", "Çıkış", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &tema_alt,
            &kose_alt,
            &PredefinedMenuItem::separator(app)?,
            &baslat,
            &ac,
            &yenile,
            &PredefinedMenuItem::separator(app)?,
            &cik,
        ],
    )?;

    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().expect("varsayilan ikon yok").clone())
        .tooltip("neon-flyout")
        .menu(&menu)
        // Sol tıklamada gösterecek bir pencere yok: kutu olaya tepki verir.
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, event| {
            let id = event.id.as_ref();
            match id {
                // Tek düzgün çıkış yolu: RunEvent::Exit → native_flyout::restore()
                "quit" => app.exit(0),
                "open" => {
                    let cfg = *app.state::<SharedConfig>().0.lock().expect("zehirli");
                    config_file::reveal(&cfg);
                }
                "reload" => {
                    let yeni = config_file::load();
                    uygula(app, &tx, yeni);
                    isaretle(&temalar, TEMALAR.map(|t| t.0), tema_id(yeni.theme));
                    isaretle(&koseler, KOSELER.map(|k| k.0), kose_id(yeni.corner));
                    let _ = baslat.set_checked(autostart::enabled());
                }
                "autostart" => {
                    // Tiklama isareti zaten degistirdi; istenen yeni durum bu.
                    let istenen = baslat.is_checked().unwrap_or(false);
                    let gercek = autostart::set(istenen);
                    // Dosya yazilamadiysa onay kutusu yalan soylemesin.
                    let _ = baslat.set_checked(gercek);

                    let mut cfg = *app.state::<SharedConfig>().0.lock().expect("zehirli");
                    cfg.autostart = gercek;
                    yaz_yetkili(app, cfg);
                }
                _ => {
                    if let Some((_, t, _)) = TEMALAR.iter().find(|(i, _, _)| *i == id) {
                        let mut cfg = *app.state::<SharedConfig>().0.lock().expect("zehirli");
                        cfg.theme = *t;
                        uygula(app, &tx, cfg);
                        isaretle(&temalar, TEMALAR.map(|x| x.0), id);
                    } else if let Some((_, c, _)) = KOSELER.iter().find(|(i, _, _)| *i == id) {
                        let mut cfg = *app.state::<SharedConfig>().0.lock().expect("zehirli");
                        cfg.corner = *c;
                        uygula(app, &tx, cfg);
                        isaretle(&koseler, KOSELER.map(|x| x.0), id);
                    }
                }
            }
        })
        .build(app)?;

    Ok(())
}

fn tema_id(t: Theme) -> &'static str {
    TEMALAR.iter().find(|(_, x, _)| *x == t).unwrap().0
}

fn kose_id(c: Corner) -> &'static str {
    KOSELER.iter().find(|(_, x, _)| *x == c).unwrap().0
}

/// Onay işaretlerini karşılıklı dışlamalı hâle getirir: yalnızca `secili` işaretli.
/// Tıklanan öğeyi Windows zaten çevirmiş oluyor; bu çağrı diğerlerini söndürür.
fn isaretle<const N: usize>(
    ogeler: &[CheckMenuItem<Wry>],
    idler: [&'static str; N],
    secili: &str,
) {
    for (oge, id) in ogeler.iter().zip(idler) {
        let _ = oge.set_checked(id == secili);
    }
}

/// Yetkili Config'i günceller, üç tüketiciye iter, dosyaya yazar — bu sırayla.
fn uygula(app: &AppHandle, tx: &Sender<DomainEvent>, cfg: Config) {
    yaz_yetkili(app, cfg);
    // 1. Tema → WebView
    let _ = app.emit(KANAL_AYAR, UiConfig { theme: cfg.theme });
    // 2. Köşe/monitör → pencere adapteri (etkisi bir sonraki gösterimde)
    window::set_placement(cfg.corner, cfg.monitor);
    // 3. Zamanlama/kural → core thread'i, mpsc üzerinden (tek yazıcı korunur)
    let _ = tx.send(DomainEvent::ConfigChanged(cfg.engine()));
}

fn yaz_yetkili(app: &AppHandle, cfg: Config) {
    {
        // `state` guard'i once bir degiskene baglanmali; dogrudan zincirlenirse
        // gecici deger kilit hala kullanilirken dusuyor (E0716).
        let state = app.state::<SharedConfig>();
        let mut g = state.0.lock().expect("zehirli");
        *g = cfg;
    }
    config_file::save(&cfg);
}
