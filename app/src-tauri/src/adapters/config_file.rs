//! Ayar dosyasının okunması ve yazılması. Karar `core/config.rs`'te; burada
//! yalnızca yol, dosya ve kabuk çağrısı var.
//!
//! **Yol `%APPDATA%\neon-flyout\config.json`.** Tauri'nin `app_config_dir()`
//! fonksiyonu bundle identifier'ı kullanıp `%APPDATA%\dev.neonflyout.app` verir;
//! kullanıcının elle düzenleyeceği bir dosya için okunaksız.

use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;

use windows::core::PCWSTR;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use crate::core::Config;

const KLASOR: &str = "neon-flyout";
const DOSYA: &str = "config.json";

fn wide(s: &std::ffi::OsStr) -> Vec<u16> {
    let mut v: Vec<u16> = s.encode_wide().collect();
    v.push(0);
    v
}

pub fn path() -> Option<PathBuf> {
    let appdata = std::env::var_os("APPDATA")?;
    Some(PathBuf::from(appdata).join(KLASOR).join(DOSYA))
}

/// Dosya yoksa **oluşturulmaz** — varsayılanlarla çalışılır. Bozuksa yine
/// varsayılan döner ve dosyaya dokunulmaz (kullanıcının yazdığı kaybolmasın).
pub fn load() -> Config {
    let Some(p) = path() else {
        eprintln!("[config] %APPDATA% okunamadi; varsayilanlar kullaniliyor");
        return Config::default();
    };
    match std::fs::read_to_string(&p) {
        Ok(s) => Config::from_json_str(&s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Config::default(),
        Err(e) => {
            eprintln!("[config] {} okunamadi ({e}); varsayilanlar", p.display());
            Config::default()
        }
    }
}

/// Klasörü gerekiyorsa oluşturur. Yazma hatası uygulamayı durdurmaz: değişiklik
/// o oturumda zaten geçerli, yalnızca kalıcı olmaz.
pub fn save(cfg: &Config) {
    let Some(p) = path() else { return };
    if let Some(dir) = p.parent() {
        if let Err(e) = std::fs::create_dir_all(dir) {
            eprintln!("[config] {} olusturulamadi ({e})", dir.display());
            return;
        }
    }
    if let Err(e) = std::fs::write(&p, cfg.to_json_string()) {
        eprintln!("[config] {} yazilamadi ({e})", p.display());
    }
}

/// "Ayar dosyasını aç". Dosya yoksa önce mevcut ayarla yazılır — kullanıcı boş
/// bir hata değil, düzenlenebilir bir dosya görsün.
pub fn reveal(cfg: &Config) {
    let Some(p) = path() else { return };
    if !p.exists() {
        save(cfg);
    }
    // Tamponlar degiskende tutuluyor: PCWSTR(wide(..).as_ptr()) yazilsa gecici
    // Vec satir sonunda duser ve isaretci sarkar.
    let yol = wide(p.as_os_str());
    let op = wide(std::ffi::OsStr::new("open"));
    unsafe {
        // Donus HINSTANCE; <= 32 hata demek ama kullaniciya gosterecegimiz bir
        // sey yok, sessizce gec.
        let _ = ShellExecuteW(
            None,
            PCWSTR(op.as_ptr()),
            PCWSTR(yol.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
    }
}
