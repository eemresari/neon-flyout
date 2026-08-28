//! P1 — SMTC spike. **Atılacak koddur** (bkz. `docs/ROADMAP.md` → P1).
//!
//! Amaç: `Windows.Media.Control` üzerinden aktif medya oturumunu okumak, oturum
//! değişimini izlemek, bir komut göndermek ve olay akışının gerçekte nasıl
//! göründüğünü kaydetmek. UI yok, Tauri yok, mimari yok.
//!
//! Olay akışının ham hâli P2'nin gürültü kurallarının girdisi: her satır zaman
//! damgalı basılır, böylece "parça geçişi kaç olay üretiyor", "abone olur olmaz
//! olay geliyor mu" sorularının cevabı tahminle değil kayıtla verilir.

use std::io::{BufRead, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use windows::core::{Ref, Result as WinResult};
use windows::Foundation::TypedEventHandler;
use windows::Media::Control::{
    CurrentSessionChangedEventArgs, GlobalSystemMediaTransportControlsSession as Session,
    GlobalSystemMediaTransportControlsSessionManager as SessionManager,
    GlobalSystemMediaTransportControlsSessionPlaybackStatus as PlaybackStatus,
    MediaPropertiesChangedEventArgs, PlaybackInfoChangedEventArgs, SessionsChangedEventArgs,
    TimelinePropertiesChangedEventArgs,
};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};

static START: OnceLock<Instant> = OnceLock::new();

/// Olay sayaçları — çıkışta özet basılır, gürültünün ölçüsü.
static N_PROPS: AtomicU64 = AtomicU64::new(0);
static N_PLAYBACK: AtomicU64 = AtomicU64::new(0);
static N_TIMELINE: AtomicU64 = AtomicU64::new(0);
static N_SESSION: AtomicU64 = AtomicU64::new(0);
static N_SESSIONS: AtomicU64 = AtomicU64::new(0);

fn log(line: &str) {
    let t = START.get().map(|s| s.elapsed().as_secs_f64()).unwrap_or(0.0);
    let mut out = std::io::stdout().lock();
    let _ = writeln!(out, "[{t:8.3}s] {line}");
    let _ = out.flush();
}

/// Bir oturuma bağlı abonelikler. Oturum değişince sökülmesi gerekiyor —
/// `docs/ARCHITECTURE.md`: "eski oturumun aboneliklerini sökmeyi unutma".
struct Bound {
    session: Session,
    props: i64,
    playback: i64,
    timeline: i64,
}

impl Bound {
    fn detach(&self) {
        let _ = self.session.RemoveMediaPropertiesChanged(self.props);
        let _ = self.session.RemovePlaybackInfoChanged(self.playback);
        let _ = self.session.RemoveTimelinePropertiesChanged(self.timeline);
    }
}

struct App {
    current: Mutex<Option<Bound>>,
}

fn status_name(s: PlaybackStatus) -> String {
    match s {
        PlaybackStatus::Closed => "Closed".into(),
        PlaybackStatus::Opened => "Opened".into(),
        PlaybackStatus::Changing => "Changing".into(),
        PlaybackStatus::Stopped => "Stopped".into(),
        PlaybackStatus::Paused => "Paused".into(),
        PlaybackStatus::Playing => "Playing".into(),
        other => format!("Bilinmeyen({})", other.0),
    }
}

fn hhmmss(ticks_100ns: i64) -> String {
    let total = (ticks_100ns / 10_000_000).max(0);
    format!("{}:{:02}", total / 60, total % 60)
}

/// SMTC pozisyonu sürekli yayınlamaz. `LastUpdatedTime`'ın yaşı, pozisyonun ne
/// kadar bayat olduğunu gösterir — interpolasyon gerekip gerekmediğinin kanıtı.
fn now_100ns_since_1601() -> i64 {
    const UNIX_TO_1601_SECS: i64 = 11_644_473_600;
    let d = SystemTime::now().duration_since(UNIX_EPOCH).unwrap_or_default();
    (d.as_secs() as i64 + UNIX_TO_1601_SECS) * 10_000_000 + (d.subsec_nanos() as i64 / 100)
}

fn describe_controls(session: &Session) -> String {
    let Ok(info) = session.GetPlaybackInfo() else {
        return "-".into();
    };
    let Ok(c) = info.Controls() else {
        return "-".into();
    };
    let mut on: Vec<&str> = Vec::new();
    for (name, val) in [
        ("play", c.IsPlayEnabled()),
        ("pause", c.IsPauseEnabled()),
        ("toggle", c.IsPlayPauseToggleEnabled()),
        ("next", c.IsNextEnabled()),
        ("prev", c.IsPreviousEnabled()),
        ("stop", c.IsStopEnabled()),
        ("seek", c.IsPlaybackPositionEnabled()),
        ("shuffle", c.IsShuffleEnabled()),
        ("repeat", c.IsRepeatEnabled()),
    ] {
        if val.unwrap_or(false) {
            on.push(name);
        }
    }
    on.join(" ")
}

/// Tek oturumun anlık görüntüsü. `full=false` ise metadata çekilmez (metadata
/// çağrısı async; olay satırlarını şişirmemek için).
fn snapshot(session: &Session, full: bool) {
    let src = session
        .SourceAppUserModelId()
        .map(|s| s.to_string())
        .unwrap_or_else(|_| "?".into());

    let status = session
        .GetPlaybackInfo()
        .and_then(|i| i.PlaybackStatus())
        .map(status_name)
        .unwrap_or_else(|_| "?".into());

    let timeline = match session.GetTimelineProperties() {
        Ok(t) => {
            let pos = t.Position().map(|p| p.Duration).unwrap_or(0);
            let end = t.EndTime().map(|p| p.Duration).unwrap_or(0);
            let age = t
                .LastUpdatedTime()
                .map(|d| (now_100ns_since_1601() - d.UniversalTime) / 10_000)
                .unwrap_or(-1);
            format!("{} / {} (yas {} ms)", hhmmss(pos), hhmmss(end), age)
        }
        Err(_) => "-".into(),
    };

    log(&format!("  kaynak    : {src}"));
    log(&format!("  durum     : {status}"));
    log(&format!("  pozisyon  : {timeline}"));
    log(&format!("  yetenekler: {}", describe_controls(session)));

    if full {
        match session.TryGetMediaPropertiesAsync().and_then(|op| op.get()) {
            Ok(p) => {
                let title = p.Title().map(|s| s.to_string()).unwrap_or_default();
                let artist = p.Artist().map(|s| s.to_string()).unwrap_or_default();
                let album = p.AlbumTitle().map(|s| s.to_string()).unwrap_or_default();
                let thumb = match p.Thumbnail() {
                    Ok(_) => "var",
                    Err(_) => "yok",
                };
                log(&format!("  baslik    : {title:?}"));
                log(&format!("  sanatci   : {artist:?}"));
                log(&format!("  album     : {album:?}"));
                log(&format!("  kapak     : {thumb}"));
            }
            Err(e) => log(&format!("  metadata  : HATA {e}")),
        }
    }
}

/// Aktif oturuma abone ol; öncekinin aboneliklerini sök.
fn rebind(app: &Arc<App>, mgr: &SessionManager) {
    let mut cur = app.current.lock().unwrap();
    if let Some(old) = cur.take() {
        old.detach();
        log("  eski oturumun abonelikleri sokuldu");
    }

    let session = match mgr.GetCurrentSession() {
        Ok(s) => s,
        Err(_) => {
            log("  aktif oturum YOK (hicbir uygulama SMTC'ye kayitli degil)");
            return;
        }
    };

    let props = session
        .MediaPropertiesChanged(&TypedEventHandler::<Session, MediaPropertiesChangedEventArgs>::new(
            |s: Ref<'_, Session>, _| {
                N_PROPS.fetch_add(1, Ordering::Relaxed);
                log("OLAY  MediaPropertiesChanged");
                if let Ok(s) = s.ok() {
                    snapshot(s, true);
                }
                Ok(())
            },
        ))
        .unwrap_or(0);

    let playback = session
        .PlaybackInfoChanged(&TypedEventHandler::<Session, PlaybackInfoChangedEventArgs>::new(
            |s: Ref<'_, Session>, _| {
                N_PLAYBACK.fetch_add(1, Ordering::Relaxed);
                let st = s
                    .ok()
                    .and_then(|s| s.GetPlaybackInfo())
                    .and_then(|i| i.PlaybackStatus())
                    .map(status_name)
                    .unwrap_or_else(|_| "?".into());
                log(&format!("OLAY  PlaybackInfoChanged      durum={st}"));
                Ok(())
            },
        ))
        .unwrap_or(0);

    let timeline = session
        .TimelinePropertiesChanged(&TypedEventHandler::<Session, TimelinePropertiesChangedEventArgs>::new(
            |s: Ref<'_, Session>, _| {
                N_TIMELINE.fetch_add(1, Ordering::Relaxed);
                let pos = s
                    .ok()
                    .and_then(|s| s.GetTimelineProperties())
                    .and_then(|t| t.Position())
                    .map(|p| hhmmss(p.Duration))
                    .unwrap_or_else(|_| "?".into());
                log(&format!("OLAY  TimelinePropertiesChanged pozisyon={pos}"));
                Ok(())
            },
        ))
        .unwrap_or(0);

    log("  yeni oturuma abone olundu");
    snapshot(&session, true);

    *cur = Some(Bound {
        session,
        props,
        playback,
        timeline,
    });
}

fn list_sessions(mgr: &SessionManager) {
    match mgr.GetSessions() {
        Ok(v) => {
            let n = v.Size().unwrap_or(0);
            log(&format!("KOMUT tum oturumlar ({n} adet)"));
            for i in 0..n {
                if let Ok(s) = v.GetAt(i) {
                    let src = s
                        .SourceAppUserModelId()
                        .map(|x| x.to_string())
                        .unwrap_or_default();
                    let st = s
                        .GetPlaybackInfo()
                        .and_then(|i| i.PlaybackStatus())
                        .map(status_name)
                        .unwrap_or_else(|_| "?".into());
                    log(&format!("  [{i}] {src}  durum={st}"));
                }
            }
        }
        Err(e) => log(&format!("KOMUT tum oturumlar HATA {e}")),
    }
}

fn with_current<F>(app: &Arc<App>, f: F)
where
    F: FnOnce(&Session),
{
    let cur = app.current.lock().unwrap();
    match cur.as_ref() {
        Some(b) => f(&b.session),
        None => log("  aktif oturum yok"),
    }
}

fn run() -> WinResult<()> {
    START.set(Instant::now()).ok();

    // Her adapter kendi apartment'ini kurar. MTA: olaylar thread pool
    // thread'lerinde gelir, mesaj pompasi gerekmez.
    unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok()? };

    log("SMTC yoneticisi isteniyor (RequestAsync)...");
    let t0 = Instant::now();
    let mgr = SessionManager::RequestAsync()?.get()?;
    log(&format!(
        "  ALINDI ({:.1} ms) — paketlenmemis exe, capability bildirimi yok",
        t0.elapsed().as_secs_f64() * 1000.0
    ));

    let app = Arc::new(App {
        current: Mutex::new(None),
    });

    // CurrentSessionChanged: Spotify <-> tarayici sekmesi gecisi burada gorunur.
    let app2 = Arc::clone(&app);
    let tok_current = mgr.CurrentSessionChanged(&TypedEventHandler::<
        SessionManager,
        CurrentSessionChangedEventArgs,
    >::new(move |m: Ref<'_, SessionManager>, _| {
        N_SESSION.fetch_add(1, Ordering::Relaxed);
        log("OLAY  CurrentSessionChanged");
        if let Ok(m) = m.ok() {
            rebind(&app2, m);
        }
        Ok(())
    }))?;

    let tok_sessions =
        mgr.SessionsChanged(&TypedEventHandler::<SessionManager, SessionsChangedEventArgs>::new(
            |m: Ref<'_, SessionManager>, _| {
                N_SESSIONS.fetch_add(1, Ordering::Relaxed);
                let n = m
                    .ok()
                    .and_then(|m| m.GetSessions())
                    .and_then(|v| v.Size())
                    .unwrap_or(0);
                log(&format!("OLAY  SessionsChanged            oturum sayisi={n}"));
                Ok(())
            },
        ))?;

    log("--- ilk baglama ---");
    rebind(&app, &mgr);
    log("--- abonelikler acik; bundan sonrasi canli olay akisi ---");
    log("komutlar: [enter]=anlik goruntu  t=toggle  n=next  p=prev  l=oturumlar  q=cikis");

    let stdin = std::io::stdin();
    for line in stdin.lock().lines() {
        let cmd = line.unwrap_or_default();
        match cmd.trim() {
            "q" => break,
            "l" => list_sessions(&mgr),
            "t" => with_current(&app, |s| {
                log("KOMUT TryTogglePlayPauseAsync");
                match s.TryTogglePlayPauseAsync().and_then(|op| op.get()) {
                    Ok(ok) => log(&format!("  sonuc={ok}")),
                    Err(e) => log(&format!("  HATA {e}")),
                }
            }),
            "n" => with_current(&app, |s| {
                log("KOMUT TrySkipNextAsync");
                match s.TrySkipNextAsync().and_then(|op| op.get()) {
                    Ok(ok) => log(&format!("  sonuc={ok}")),
                    Err(e) => log(&format!("  HATA {e}")),
                }
            }),
            "p" => with_current(&app, |s| {
                log("KOMUT TrySkipPreviousAsync");
                match s.TrySkipPreviousAsync().and_then(|op| op.get()) {
                    Ok(ok) => log(&format!("  sonuc={ok}")),
                    Err(e) => log(&format!("  HATA {e}")),
                }
            }),
            _ => with_current(&app, |s| {
                log("KOMUT anlik goruntu");
                snapshot(s, true);
            }),
        }
    }

    log(&format!(
        "olay sayaclari: props={} playback={} timeline={} currentSession={} sessions={}",
        N_PROPS.load(Ordering::Relaxed),
        N_PLAYBACK.load(Ordering::Relaxed),
        N_TIMELINE.load(Ordering::Relaxed),
        N_SESSION.load(Ordering::Relaxed),
        N_SESSIONS.load(Ordering::Relaxed),
    ));

    if let Some(b) = app.current.lock().unwrap().take() {
        b.detach();
    }
    let _ = mgr.RemoveCurrentSessionChanged(tok_current);
    let _ = mgr.RemoveSessionsChanged(tok_sessions);
    log("cikildi");
    Ok(())
}

fn main() {
    if let Err(e) = run() {
        eprintln!("HATA: {e}  (HRESULT {:#010x})", e.code().0);
        std::process::exit(1);
    }
}
