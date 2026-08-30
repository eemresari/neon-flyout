//! "Windows ile başlat" — Başlangıç klasörüne kısayol.
//!
//! **Gerçek kaynak dosyanın kendisidir**, `config.json` değil: kullanıcı kısayolu
//! klasörden elle silerse tepsi menüsü doğruyu gösterir.
//!
//! Dev'de kısayol **debug exe'sini** gösterir; `npm run tauri dev` ile denerken
//! bu beklenen davranış.

use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;

use windows::core::{Interface, PCWSTR};
use windows::Win32::Foundation::E_FAIL;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, IPersistFile, CLSCTX_INPROC_SERVER,
    COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Shell::{
    IShellLinkW, SHGetKnownFolderPath, ShellLink, FOLDERID_Startup, KF_FLAG_DEFAULT,
};

const KISAYOL: &str = "neon-flyout.lnk";

fn wide(s: &std::ffi::OsStr) -> Vec<u16> {
    let mut v: Vec<u16> = s.encode_wide().collect();
    v.push(0);
    v
}

/// Başlangıç klasörü. Sabit yol yerine `SHGetKnownFolderPath` — yerelleştirilmiş
/// veya yönlendirilmiş kurulumlarda da doğru olsun diye.
fn startup_dir() -> Option<PathBuf> {
    unsafe {
        let p = SHGetKnownFolderPath(&FOLDERID_Startup, KF_FLAG_DEFAULT, None).ok()?;
        let s = p.to_string().ok()?;
        CoTaskMemFree(Some(p.0 as *const _));
        Some(PathBuf::from(s))
    }
}

fn kisayol_yolu() -> Option<PathBuf> {
    Some(startup_dir()?.join(KISAYOL))
}

pub fn enabled() -> bool {
    kisayol_yolu().is_some_and(|p| p.exists())
}

/// `true` istenirse kısayolu yazar, `false` istenirse siler. Dönüş **gerçekleşen**
/// durumdur — başarısızlıkta menü yanlış onay kutusu göstermesin.
pub fn set(on: bool) -> bool {
    let Some(p) = kisayol_yolu() else {
        return enabled();
    };
    if on {
        if let Err(e) = yaz(&p) {
            eprintln!("[autostart] kisayol yazilamadi ({e})");
        }
    } else if let Err(e) = std::fs::remove_file(&p) {
        if e.kind() != std::io::ErrorKind::NotFound {
            eprintln!("[autostart] kisayol silinemedi ({e})");
        }
    }
    enabled()
}

fn yaz(hedef: &std::path::Path) -> windows::core::Result<()> {
    let exe = std::env::current_exe()
        .map_err(|e| windows::core::Error::new(E_FAIL, e.to_string()))?;
    let dir = exe.parent().unwrap_or(&exe).to_path_buf();

    // Omur tuzagi: PCWSTR(wide(..).as_ptr()) yazilsa gecici Vec satir sonunda
    // duser ve isaretci sarkar — derleyici bunu YAKALAMAZ. Tamponlar once
    // degiskende tutuluyor.
    let exe_w = wide(exe.as_os_str());
    let dir_w = wide(dir.as_os_str());
    let desc_w = wide(std::ffi::OsStr::new("neon-flyout — medya flyout'u"));
    let hedef_w = wide(hedef.as_os_str());

    unsafe {
        // Tauri'nin ana thread'i zaten bir apartment'ta; RPC_E_CHANGED_MODE
        // beklenen ve zararsiz, yut.
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        link.SetPath(PCWSTR(exe_w.as_ptr()))?;
        link.SetWorkingDirectory(PCWSTR(dir_w.as_ptr()))?;
        link.SetDescription(PCWSTR(desc_w.as_ptr()))?;

        let file: IPersistFile = link.cast()?;
        file.Save(PCWSTR(hedef_w.as_ptr()), true)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// COM yolunu uçtan uca sınar: **gerçek Başlangıç klasörüne yazar.**
    /// Bu yüzden `#[ignore]` — varsayılan takımda çalışmaz.
    ///
    /// Elle çalıştır: `cargo test --lib autostart -- --ignored --nocapture`
    ///
    /// Uyarı: kısayolun hedefi test ikilisidir, uygulamanın exe'si değil. Test
    /// kendini temizler; yarıda kalırsa Başlangıç klasöründen `neon-flyout.lnk`
    /// dosyasını elle sil.
    #[test]
    #[ignore]
    fn kisayol_yazilip_silinebiliyor() {
        let p = kisayol_yolu().expect("Baslangic klasoru cozulemedi");
        println!("kisayol yolu: {}", p.display());

        let onceden_vardi = enabled();

        let acik = set(true);
        let dosya_var = p.exists();
        // Temizlik once: assert patlarsa bile klasorde iz kalmasin.
        if !onceden_vardi {
            set(false);
        }

        assert!(acik, "set(true) kisayolu olusturamadi");
        assert!(dosya_var, "kisayol dosyasi diskte yok");
        if !onceden_vardi {
            assert!(!enabled(), "set(false) kisayolu silemedi");
        }
    }
}
