//! L2 — WindowAdapter. Pencere stilleri, konum, z-order, göster/gizle.
//!
//! `SW_HIDE` bilinçli bir karar: oyunun üstünde görünen her piksel DWM'in Direct
//! Flip yolunu kapatıyor, opaklığı sıfırlayıp pencereyi açık bırakmak bedeli
//! sürdürüyor (`CLAUDE.md` → "Neden `SW_HIDE`, alpha 0 değil").

use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, ERROR_SUCCESS, HANDLE, HWND, MAX_PATH, RECT};
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromWindow, MONITORINFO, MONITOR_DEFAULTTOPRIMARY,
};
use windows::Win32::Storage::Packaging::Appx::GetApplicationUserModelId;
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowLongPtrW, GetWindowRect, GetWindowThreadProcessId,
    SetWindowLongPtrW, SetWindowPos, ShowWindow, GWL_EXSTYLE, HWND_TOPMOST, SWP_NOACTIVATE,
    SWP_NOSIZE, SW_HIDE, SW_SHOWNOACTIVATE, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
};

/// Ekran kenarına bırakılan boşluk (fiziksel piksel). Köşe/monitör seçimi P3.
const MARGIN: i32 = 16;

fn hwnd(raw: isize) -> HWND {
    HWND(raw as *mut std::ffi::c_void)
}

/// Tauri'nin `focus: false` ayarı tek başına odak çalmayı engellemiyor;
/// `WS_EX_NOACTIVATE` şart. `WS_EX_TOOLWINDOW` Alt-Tab listesinden,
/// `WS_EX_TRANSPARENT` fare olaylarından çıkarır (P2'de kutu tıklanabilir değil).
pub fn apply_styles(raw: isize) {
    unsafe {
        let cur = GetWindowLongPtrW(hwnd(raw), GWL_EXSTYLE);
        let want = cur
            | WS_EX_NOACTIVATE.0 as isize
            | WS_EX_TOOLWINDOW.0 as isize
            | WS_EX_TRANSPARENT.0 as isize;
        SetWindowLongPtrW(hwnd(raw), GWL_EXSTYLE, want);
    }
}

/// Pencereyi sağ üst köşeye taşır. Ölçüler fiziksel pikselde okunuyor, bu yüzden
/// DPI ölçeklemesi ayrıca hesaplanmıyor.
pub fn place_top_right(raw: isize) {
    unsafe {
        let mut r = RECT::default();
        if GetWindowRect(hwnd(raw), &mut r).is_err() {
            return;
        }
        let w = r.right - r.left;

        let mon = MonitorFromWindow(hwnd(raw), MONITOR_DEFAULTTOPRIMARY);
        let mut mi = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(mon, &mut mi).as_bool() {
            return;
        }
        let work = mi.rcWork;

        let _ = SetWindowPos(
            hwnd(raw),
            Some(HWND_TOPMOST),
            work.right - w - MARGIN,
            work.top + MARGIN,
            0,
            0,
            SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}

/// Göster. Her göstermede z-order tazelenir; `SWP_NOACTIVATE` ve
/// `SW_SHOWNOACTIVATE` odak çalmayı engeller.
pub fn show(raw: isize) {
    place_top_right(raw);
    unsafe {
        let _ = ShowWindow(hwnd(raw), SW_SHOWNOACTIVATE);
    }
}

/// Gerçek `SW_HIDE` — opaklık sıfırlamak değil.
pub fn hide(raw: isize) {
    unsafe {
        let _ = ShowWindow(hwnd(raw), SW_HIDE);
    }
}

/// 4. gürültü kuralının girdisi: SMTC kaynağı şu an ön planda mı?
///
/// Paketli uygulamalarda AUMID birebir karşılaştırılır. Paketsiz kaynaklarda
/// (tarayıcılar) SMTC'nin verdiği kimlik bir kabuk AppID'si —
/// `OperaSoftware.OperaGXWebBrowser.1783353490` — süreç AUMID'si değil; orada
/// exe adının kimlikte geçip geçmediğine bakılıyor. **Sezgisel**; kural bu
/// yüzden varsayılan olarak kapalı.
pub fn foreground_matches(source_id: &str) -> bool {
    if source_id.is_empty() {
        return false;
    }
    unsafe {
        let fg = GetForegroundWindow();
        if fg.0.is_null() {
            return false;
        }
        let mut pid = 0u32;
        GetWindowThreadProcessId(fg, Some(&mut pid));
        if pid == 0 {
            return false;
        }
        let Ok(proc) = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) else {
            return false;
        };
        let matched = aumid(proc).is_some_and(|a| a.eq_ignore_ascii_case(source_id))
            || exe_stem(proc).is_some_and(|s| {
                !s.is_empty()
                    && source_id
                        .to_ascii_lowercase()
                        .contains(&s.to_ascii_lowercase())
            });
        let _ = CloseHandle(proc);
        matched
    }
}

unsafe fn aumid(proc: HANDLE) -> Option<String> {
    let mut len = MAX_PATH * 2;
    let mut buf = vec![0u16; len as usize];
    let err = GetApplicationUserModelId(proc, &mut len, Some(PWSTR(buf.as_mut_ptr())));
    if err != ERROR_SUCCESS {
        return None; // paketsiz süreç: APPMODEL_ERROR_NO_APPLICATION
    }
    buf.truncate(len.saturating_sub(1) as usize);
    Some(String::from_utf16_lossy(&buf))
}

unsafe fn exe_stem(proc: HANDLE) -> Option<String> {
    let mut len = MAX_PATH;
    let mut buf = vec![0u16; len as usize];
    QueryFullProcessImageNameW(proc, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len).ok()?;
    buf.truncate(len as usize);
    let path = String::from_utf16_lossy(&buf);
    let file = path.rsplit(['\\', '/']).next()?;
    Some(file.trim_end_matches(".exe").to_string())
}
