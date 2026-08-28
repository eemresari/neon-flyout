//! L2 — SmtcAdapter. `Windows.Media.Control` oturumunu dinler, olayı domain
//! tipine çevirip mpsc'ye yazar.
//!
//! **Durum tutmaz.** Burada tutulan tek şey abonelik token'ları — oturum
//! değişince sökülebilsinler diye. Oynatıcı durumu core'daki tek yazıcı görevde
//! yaşar; olaylar eşzamanlı thread pool thread'lerinde geliyor
//! (`docs/P1-SONUC.md` § 7).
//!
//! `TimelinePropertiesChanged`'e **abone olunmuyor**: tetik değil, çalma
//! duraklatılmışken bile geliyor (§ 6.5). Pozisyon her anlık görüntüde taze
//! okunuyor, ayrı olaya gerek yok.

use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use windows::core::{Ref, Result as WinResult};
use windows::Foundation::TypedEventHandler;
use windows::Media::Control::{
    CurrentSessionChangedEventArgs, GlobalSystemMediaTransportControlsSession as Session,
    GlobalSystemMediaTransportControlsSessionManager as SessionManager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as WinStatus,
    MediaPropertiesChangedEventArgs, PlaybackInfoChangedEventArgs,
};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

use crate::adapters::window;
use crate::core::model::{Controls, MediaSnapshot, PlaybackState, Position, Track};
use crate::core::DomainEvent;

/// Bir oturuma bağlı abonelikler. Oturum değişince sökülmezse ölmüş oturumdan
/// olay almaya devam ederiz.
struct Bound {
    session: Session,
    props: i64,
    playback: i64,
}

impl Bound {
    fn detach(&self) {
        let _ = self.session.RemoveMediaPropertiesChanged(self.props);
        let _ = self.session.RemovePlaybackInfoChanged(self.playback);
    }
}

/// Kendi thread'i, kendi apartment'ı. MTA: olaylar thread pool'da gelir, mesaj
/// pompası gerekmez.
pub fn spawn(tx: Sender<DomainEvent>) -> std::io::Result<()> {
    std::thread::Builder::new().name("smtc".into()).spawn(move || {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
        if let Err(e) = run(tx) {
            eprintln!("[smtc] baslatilamadi: {e} (HRESULT {:#010x})", e.code().0);
        }
    })?;
    Ok(())
}

fn run(tx: Sender<DomainEvent>) -> WinResult<()> {
    // Paketlenmemiş exe'de capability bildirimi gerekmiyor; soğuk çağrı ~30 ms
    // ve uygulama açılışında bir kez ödeniyor (`docs/P1-SONUC.md` § 1).
    let mgr = SessionManager::RequestAsync()?.get()?;
    let bound: Arc<Mutex<Option<Bound>>> = Arc::new(Mutex::new(None));

    let b = Arc::clone(&bound);
    let t = tx.clone();
    mgr.CurrentSessionChanged(&TypedEventHandler::<
        SessionManager,
        CurrentSessionChangedEventArgs,
    >::new(move |m: Ref<'_, SessionManager>, _| {
        if let Ok(m) = m.ok() {
            rebind(&b, m, &t);
        }
        Ok(())
    }))?;

    rebind(&bound, &mgr, &tx);

    // Olaylar thread pool'da geliyor; bu thread yalnızca `mgr` ve abonelikleri
    // hayatta tutmak için duruyor.
    loop {
        std::thread::park();
    }
}

/// Aktif oturuma abone ol, öncekinin aboneliklerini sök.
fn rebind(bound: &Arc<Mutex<Option<Bound>>>, mgr: &SessionManager, tx: &Sender<DomainEvent>) {
    let mut guard = match bound.lock() {
        Ok(g) => g,
        Err(p) => p.into_inner(),
    };
    if let Some(old) = guard.take() {
        old.detach();
    }

    let Ok(session) = mgr.GetCurrentSession() else {
        let _ = tx.send(DomainEvent::SessionGone);
        return;
    };

    let source_id = session
        .SourceAppUserModelId()
        .map(|s| s.to_string())
        .unwrap_or_default();
    // Tek başına kutuyu açmaz — yeni oturum boş doğuyor (§ 5.1).
    let _ = tx.send(DomainEvent::SessionSwitched { source_id });

    let t = tx.clone();
    let props = session
        .MediaPropertiesChanged(&TypedEventHandler::<Session, MediaPropertiesChangedEventArgs>::new(
            move |s: Ref<'_, Session>, _| {
                if let Ok(s) = s.ok() {
                    if let Some(snap) = snapshot(s) {
                        let _ = t.send(DomainEvent::Media(snap));
                    }
                }
                Ok(())
            },
        ))
        .unwrap_or(0);

    let t = tx.clone();
    let playback = session
        .PlaybackInfoChanged(&TypedEventHandler::<Session, PlaybackInfoChangedEventArgs>::new(
            move |s: Ref<'_, Session>, _| {
                if let Ok(s) = s.ok() {
                    if let Some(snap) = snapshot(s) {
                        let _ = t.send(DomainEvent::Media(snap));
                    }
                }
                Ok(())
            },
        ))
        .unwrap_or(0);

    // Bağlanma anında içerik hazırsa gönder (tarayıcı → Spotify yönünde oluyor).
    // Boşsa gönderme: içerik 14 ms sonra `MediaPropertiesChanged` ile gelecek.
    if let Some(snap) = snapshot(&session) {
        if snap.has_content() {
            let _ = tx.send(DomainEvent::Media(snap));
        }
    }

    *guard = Some(Bound { session, props, playback });
}

/// Oturumun o andaki hâli. Ölmekte olan oturumdan okumak hata verebilir —
/// hata değil, sessizce atılır (§ 7).
fn snapshot(session: &Session) -> Option<MediaSnapshot> {
    let info = session.GetPlaybackInfo().ok()?;
    let state = info.PlaybackStatus().map(to_state).unwrap_or(PlaybackState::Unknown);

    let controls = info
        .Controls()
        .map(|c| Controls {
            play: c.IsPlayEnabled().unwrap_or(false),
            pause: c.IsPauseEnabled().unwrap_or(false),
            toggle: c.IsPlayPauseToggleEnabled().unwrap_or(false),
            next: c.IsNextEnabled().unwrap_or(false),
            prev: c.IsPreviousEnabled().unwrap_or(false),
            stop: c.IsStopEnabled().unwrap_or(false),
            seek: c.IsPlaybackPositionEnabled().unwrap_or(false),
        })
        .unwrap_or_default();

    let source_id = session
        .SourceAppUserModelId()
        .map(|s| s.to_string())
        .unwrap_or_default();

    let track = session
        .TryGetMediaPropertiesAsync()
        .and_then(|op| op.get())
        .map(|p| Track {
            title: p.Title().map(|s| s.to_string()).unwrap_or_default(),
            artist: p.Artist().map(|s| s.to_string()).unwrap_or_default(),
            album: p.AlbumTitle().map(|s| s.to_string()).unwrap_or_default(),
        })
        .unwrap_or_default();

    Some(MediaSnapshot {
        source_is_foreground: window::foreground_matches(&source_id),
        source_id,
        track,
        state,
        position: position(session),
        controls,
    })
}

/// `LastUpdatedTime == 0` → pozisyon bilinmiyor. Yoksa yaş 425 yıl çıkıyor ve
/// interpolasyon saçmalıyor (`docs/P1-SONUC.md` § 5.2).
fn position(session: &Session) -> Option<Position> {
    let t = session.GetTimelineProperties().ok()?;
    let updated = t.LastUpdatedTime().ok()?.UniversalTime;
    if updated == 0 {
        return None;
    }
    let ticks_to_ms = |t: i64| (t.max(0) / 10_000) as u64;
    Some(Position {
        position_ms: ticks_to_ms(t.Position().ok()?.Duration),
        duration_ms: ticks_to_ms(t.EndTime().ok()?.Duration),
        age_ms: ticks_to_ms(now_100ns_since_1601() - updated),
    })
}

fn now_100ns_since_1601() -> i64 {
    const UNIX_TO_1601_SECS: i64 = 11_644_473_600;
    let d = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    (d.as_secs() as i64 + UNIX_TO_1601_SECS) * 10_000_000 + (d.subsec_nanos() as i64 / 100)
}

fn to_state(s: WinStatus) -> PlaybackState {
    match s {
        WinStatus::Closed => PlaybackState::Closed,
        WinStatus::Opened => PlaybackState::Opened,
        WinStatus::Changing => PlaybackState::Changing,
        WinStatus::Stopped => PlaybackState::Stopped,
        WinStatus::Paused => PlaybackState::Paused,
        WinStatus::Playing => PlaybackState::Playing,
        _ => PlaybackState::Unknown,
    }
}
