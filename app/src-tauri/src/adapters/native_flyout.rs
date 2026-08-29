//! L2 — NativeFlyoutSuppressor. Windows'un kendi ses/medya kutusunu bastırır.
//!
//! **Emniyet kemeri, özellik değil.** Bu makinede bastırılacak pencere yok:
//! Windows kutusunu ses/medya *tuşuna basılmasına* açıyor, seviyenin değişmesine
//! değil — kullanıcı sesi fareyle değiştirdiği için Windows'un kutusu hiç
//! çıkmıyor (`docs/P4-ARASTIRMA.md` § 3–4, 45 sn pasif izlemeyle ölçüldü).
//! Kod yine de duruyor: tuş kullanan bir makinede pencere gerçekten çıkar.
//!
//! Bulamazsa **sessizce vazgeçer** — yol haritasının P4 çıkış kriteri bu.
//! Yeniden deneme, kanca, zamanlayıcı yok; ModernFlyouts'un 3 sn'lik rehook
//! döngüsü bastırmanın çalışmasına bağımlı bir uygulama için, biz değiliz.
//!
//! ---
//! ⚠ **BU DOSYA HENÜZ DERLENMEDİ VE `adapters/mod.rs`'e BAĞLANMADI.**
//! Smart App Control derlemeyi engellediği için (`CLAUDE.md` → SAC) doğrulanamadı.
//! Engel kalkınca sırasıyla: `mod.rs`'e `pub mod native_flyout;` ekle →
//! `cargo test --lib` ile derle → `lib.rs` `setup`'ında `suppress()`, çıkışta
//! `restore()` çağır → `spikes/p4-probe/p4-probe.ps1` ile öncesi/sonrası doğrula.
//! Bu makinede beklenen sonuç: `suppress()` **false** döner, uygulama normal
//! çalışır — P4 çıkış kriterinin kendisi bu.

use std::sync::Mutex;

use windows::Win32::Foundation::{BOOL, CloseHandle, HWND, MAX_PATH};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowExW, GetWindowLongPtrW, GetWindowThreadProcessId, SetWindowLongPtrW, SetWindowPos,
    ShowWindow, GWL_EXSTYLE, SWP_NOACTIVATE, SWP_NOSIZE, SWP_NOZORDER, SW_MINIMIZE, SW_RESTORE,
    WS_EX_LAYERED, WS_EX_TRANSPARENT,
};
use windows::core::{PCWSTR, PWSTR};

// `GetWindowBand` belgelenmemiş bir `user32` ihracı ve `windows` crate'inde yok;
// elle bildiriliyor. Pencerenin z-band'ini okur.
#[link(name = "user32")]
extern "system" {
    fn GetWindowBand(hwnd: HWND, pdwband: *mut u32) -> BOOL;
}

/// Flyout host'unun bandı. **Ayırt etmenin tek güvenilir yolu bu.**
///
/// `XamlExplorerHostIslandWindow` kabuğun genel amaçlı XAML ada penceresi; bu
/// makinedeki tek örneği **Alt+Tab** (band 16 = `SystemTools`). FluentFlyout'un
/// bant kontrolsüz sezgiseli (InputSite çocuğu + sıfır olmayan rect) o pencereye
/// tam uyuyor — kopyalansaydı Alt+Tab'ı ekran dışına taşırdık
/// (`docs/P4-ARASTIRMA.md` § 3.1). **Bu şart kaldırılamaz.**
const ZBID_ABOVELOCK_UX: u32 = 0x12;

/// 22H2 (build 22620) OSD'yi XAML adasına taşıdı; öncesi klasik DirectUI.
const BUILD_22H2: u32 = 22620;

/// Bastırılan pencerenin geri alınması için gereken her şey. `None` = bastırma
/// yapılmadı (pencere bulunamadı) ya da zaten geri alındı.
static SUPPRESSED: Mutex<Option<Suppressed>> = Mutex::new(None);

struct Suppressed {
    host: isize,
    ex_style: isize,
}

// SAFETY: yalnızca HWND'nin ham değeri saklanıyor, COM/thread benzeşimli bir
// nesne değil; ShowWindow/SetWindowPos herhangi bir thread'den çağrılabilir.
unsafe impl Send for Suppressed {}

fn hwnd(raw: isize) -> HWND {
    HWND(raw as *mut std::ffi::c_void)
}

fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Windows'un flyout'unu ara; bulursan ekran dışına taşı ve küçült.
///
/// Bulamazsa hiçbir şey yapmaz — çağıran tarafın kontrol etmesi gereken bir hata
/// değil, beklenen sonuç. Döndürdüğü değer yalnızca günlük/ayar içindir.
pub fn suppress() -> bool {
    let Some((host, dui)) = find() else {
        return false;
    };

    unsafe {
        let ex_style = GetWindowLongPtrW(hwnd(host), GWL_EXSTYLE);
        // Tıklama geçirgen + katmanlı: ekran dışında olsa bile masaüstünün
        // hit-test bölgesini sahiplenmesin.
        SetWindowLongPtrW(
            hwnd(host),
            GWL_EXSTYLE,
            ex_style | WS_EX_LAYERED.0 as isize | WS_EX_TRANSPARENT.0 as isize,
        );
        let _ = SetWindowPos(
            hwnd(host),
            None,
            -99_999,
            -99_999,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
        let _ = ShowWindow(hwnd(dui), SW_MINIMIZE);
        let _ = ShowWindow(hwnd(host), SW_MINIMIZE);

        if let Ok(mut g) = SUPPRESSED.lock() {
            *g = Some(Suppressed { host, ex_style });
        }
    }
    true
}

/// Bastırmayı geri al. Kullanıcının masaüstünde kalıcı bir değişiklik bırakmıyoruz;
/// çıkışta çağrılır. Bastırma yapılmadıysa hiçbir şey yapmaz.
pub fn restore() {
    let Ok(mut g) = SUPPRESSED.lock() else {
        return;
    };
    let Some(s) = g.take() else {
        return;
    };
    unsafe {
        SetWindowLongPtrW(hwnd(s.host), GWL_EXSTYLE, s.ex_style);
        let _ = SetWindowPos(
            hwnd(s.host),
            None,
            0,
            0,
            0,
            0,
            SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
        );
        let _ = ShowWindow(hwnd(s.host), SW_RESTORE);
    }
}

/// Flyout host'unu ve iç penceresini bul. Üç şart birden tutmalı: sınıf çifti,
/// sahip süreç `explorer.exe`, band `AboveLockUX`.
fn find() -> Option<(isize, isize)> {
    let win11 = build_number() >= BUILD_22H2;
    let (outer, inner_class, inner_title) = if win11 {
        (
            "XamlExplorerHostIslandWindow",
            "Windows.UI.Composition.DesktopWindowContentBridge",
            "DesktopWindowXamlSource",
        )
    } else {
        ("NativeHWNDHost", "DirectUIHWND", "")
    };

    let outer_w = wide(outer);
    let inner_class_w = wide(inner_class);
    let inner_title_w = wide(inner_title);
    let empty = wide("");

    let mut host = HWND::default();
    loop {
        host = unsafe {
            FindWindowExW(
                None,
                Some(host),
                PCWSTR(outer_w.as_ptr()),
                PCWSTR(empty.as_ptr()),
            )
        }
        .ok()?;

        let inner = unsafe {
            FindWindowExW(
                Some(host),
                None,
                PCWSTR(inner_class_w.as_ptr()),
                PCWSTR(inner_title_w.as_ptr()),
            )
        };
        let Ok(inner) = inner else { continue };

        if !owner_is_explorer(host) {
            continue;
        }

        let mut band = 0u32;
        let ok = unsafe { GetWindowBand(host, &mut band) }.as_bool();
        if ok && band == ZBID_ABOVELOCK_UX {
            return Some((host.0 as isize, inner.0 as isize));
        }
    }
}

fn owner_is_explorer(h: HWND) -> bool {
    unsafe {
        let mut pid = 0u32;
        GetWindowThreadProcessId(h, Some(&mut pid));
        if pid == 0 {
            return false;
        }
        let Ok(proc) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return false;
        };
        let mut len = MAX_PATH;
        let mut buf = vec![0u16; len as usize];
        let ok =
            QueryFullProcessImageNameW(proc, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len)
                .is_ok();
        let _ = CloseHandle(proc);
        if !ok {
            return false;
        }
        buf.truncate(len as usize);
        String::from_utf16_lossy(&buf)
            .rsplit(['\\', '/'])
            .next()
            .is_some_and(|f| f.eq_ignore_ascii_case("explorer.exe"))
    }
}

/// `RtlGetVersion` yerine `GetVersionEx`'in manifest tuzağına düşmemek için
/// sürüm kabuğun kendi penceresinden değil, `ntdll` üzerinden okunuyor.
fn build_number() -> u32 {
    use windows::Wdk::System::SystemServices::RtlGetVersion;
    use windows::Win32::System::SystemInformation::OSVERSIONINFOW;

    let mut vi = OSVERSIONINFOW {
        dwOSVersionInfoSize: std::mem::size_of::<OSVERSIONINFOW>() as u32,
        ..Default::default()
    };
    if unsafe { RtlGetVersion(&mut vi) }.is_ok() {
        vi.dwBuildNumber
    } else {
        0 // bilinmiyorsa eski yol denenir; yanlış sınıfta zaten hiçbir şey bulunmaz
    }
}
