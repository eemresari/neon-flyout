//! Tek yazıcı görev. Adapter'lardan gelen olayları alır, durumu günceller,
//! gürültü kurallarını uygular, görünürlük makinesini sürer ve **yan etkisi
//! olmayan** bir efekt listesi döndürür. Pencereyi gösterme/gizleme ve IPC
//! gönderme işini sürücü (`lib.rs`) yapar; bu sayede core test edilebilir kalır.

use serde::Serialize;

use super::event::DomainEvent;
use super::model::{FlyoutKind, MediaSnapshot, PlayerState};
use super::rules::{Gatekeeper, RuleConfig, Verdict};
use super::visibility::{Expiry, ShowOutcome, VisibilityMachine, COALESCE_MS, FADE_MS, SHOW_MS};

/// WebView'e gönderilen kare. `docs/DESIGN.md`'deki DOM'a P3'te bağlanacak;
/// P2'de aynı alanlar çirkin bir kutuya basılıyor.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Phase {
    /// Kutu ekrana geliyor — giriş animasyonu oynasın.
    Enter,
    /// Kutu zaten ekranda — sadece içerik değişsin, animasyon oynamasın.
    Update,
    /// Süre doldu — çıkış animasyonu. Bitiminde pencere gerçekten gizlenir.
    Leave,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Frame {
    pub kind: FlyoutKind,
    pub phase: Phase,
    /// Bu olay bir öncekiyle aynı kullanıcı eyleminden mi geliyor
    /// (`docs/P1-SONUC.md` § 6.3'teki 811 ms'lik seri).
    pub same_burst: bool,
    pub state: PlayerState,
}

/// Sürücünün uygulayacağı yan etkiler. Sıra önemli: içerik **önce** gider,
/// pencere **sonra** açılır (`docs/ARCHITECTURE.md`: emit → WebView → SW_SHOW).
#[derive(Clone, Debug, PartialEq)]
pub enum Effect {
    Emit(Frame),
    Show,
    Hide,
}

#[derive(Clone, Copy, Debug)]
pub struct EngineConfig {
    pub show_ms: u64,
    pub fade_ms: u64,
    pub coalesce_ms: u64,
    pub rules: RuleConfig,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            show_ms: SHOW_MS,
            fade_ms: FADE_MS,
            coalesce_ms: COALESCE_MS,
            rules: RuleConfig::default(),
        }
    }
}

pub struct Engine {
    state: PlayerState,
    rules: Gatekeeper,
    vis: VisibilityMachine,
    /// Ekranda olan (ya da en son olan) kutu türü.
    kind: FlyoutKind,
}

impl Default for Engine {
    fn default() -> Self {
        Self::new(EngineConfig::default())
    }
}

impl Engine {
    pub fn new(cfg: EngineConfig) -> Self {
        Self {
            state: PlayerState::default(),
            rules: Gatekeeper::new(cfg.rules),
            vis: VisibilityMachine::new(cfg.show_ms, cfg.fade_ms, cfg.coalesce_ms),
            kind: FlyoutKind::Media,
        }
    }

    /// Testlerde ve P3'te (WebView yeniden yuklendiginde durumu geri vermek)
    /// kullaniliyor.
    #[allow(dead_code)]
    pub fn state(&self) -> &PlayerState {
        &self.state
    }

    #[allow(dead_code)]
    pub fn is_on_screen(&self) -> bool {
        self.vis.is_on_screen()
    }

    /// Bir sonraki uyanmaya kaç ms kaldı. `None` ise sürücü süresiz bekleyebilir.
    pub fn next_timeout(&self, now_ms: u64) -> Option<u64> {
        self.vis.next_timeout(now_ms)
    }

    pub fn handle(&mut self, ev: DomainEvent, now_ms: u64) -> Vec<Effect> {
        let changed = self.merge(&ev);
        let verdict = self.rules.judge(&ev);
        let ev_kind = kind_of(&ev);

        match (verdict, ev_kind) {
            (Verdict::Trigger, Some(kind)) => {
                self.kind = kind;
                let (outcome, same_burst) = self.vis.trigger(now_ms);
                let phase = match outcome {
                    ShowOutcome::Opened | ShowOutcome::Resumed => Phase::Enter,
                    ShowOutcome::UpdatedInPlace => Phase::Update,
                };
                let mut fx = vec![Effect::Emit(self.frame(phase, same_burst))];
                if outcome == ShowOutcome::Opened {
                    fx.push(Effect::Show);
                }
                fx
            }
            // Tetik değil ama içerik değişti: kutu ekrandaysa **yerinde** güncelle.
            // Deadline ileri alınmaz — kutuyu bu olay açmadı, uzatmaya da hakkı yok.
            _ if changed && self.vis.is_on_screen() && ev_kind == Some(self.kind) => {
                vec![Effect::Emit(self.frame(Phase::Update, true))]
            }
            // Gizliyken WebView'e hiçbir şey gönderilmez (`docs/ARCHITECTURE.md` L4).
            _ => Vec::new(),
        }
    }

    /// Deadline kontrolü. Sürücü döngü her uyanışta çağırır.
    pub fn tick(&mut self, now_ms: u64) -> Vec<Effect> {
        match self.vis.tick(now_ms) {
            Some(Expiry::StartFadeOut) => vec![Effect::Emit(self.frame(Phase::Leave, false))],
            Some(Expiry::Hide) => vec![Effect::Hide],
            None => Vec::new(),
        }
    }

    fn frame(&self, phase: Phase, same_burst: bool) -> Frame {
        Frame {
            kind: self.kind,
            phase,
            same_burst,
            state: self.state.clone(),
        }
    }

    /// Olayı duruma işler; durum gerçekten değiştiyse `true`.
    fn merge(&mut self, ev: &DomainEvent) -> bool {
        let before = self.state.clone();
        match ev {
            DomainEvent::Media(s) => self.merge_media(s),
            DomainEvent::Volume(v) => self.state.volume = Some(v.clone()),
            // Yeni oturum boş doğuyor (§ 5.1). Kaynak adını hemen yazmak, içerik
            // 14 ms sonra gelene kadar "yeni kaynak + eski parça" karması gösterir.
            // Bu yüzden oturum değişimi duruma dokunmaz.
            DomainEvent::SessionSwitched { .. } => {}
            DomainEvent::SessionGone => self.state.media = None,
        }
        before != self.state
    }

    fn merge_media(&mut self, s: &MediaSnapshot) {
        let bos_dogan = !s.has_content() && !s.state.is_showable();
        match (&mut self.state.media, bos_dogan) {
            // Boş doğan oturumun anlık görüntüsü ekranda duran içeriği silmemeli;
            // sadece durum ve kaynak kimliği tazelenir.
            (Some(cur), true) => {
                cur.source_id = s.source_id.clone();
                cur.state = s.state;
            }
            _ => self.state.media = Some(s.clone()),
        }
    }
}

fn kind_of(ev: &DomainEvent) -> Option<FlyoutKind> {
    match ev {
        DomainEvent::Media(_) => Some(FlyoutKind::Media),
        DomainEvent::Volume(_) => Some(FlyoutKind::Volume),
        DomainEvent::SessionSwitched { .. } | DomainEvent::SessionGone => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::model::{PlaybackState, Track, VolumeSnapshot};

    fn media(title: &str, state: PlaybackState) -> DomainEvent {
        DomainEvent::Media(MediaSnapshot {
            source_id: "Spotify".into(),
            track: Track {
                title: title.into(),
                artist: "Vacations".into(),
                album: String::new(),
            },
            state,
            ..Default::default()
        })
    }

    fn vol(level: f32) -> DomainEvent {
        DomainEvent::Volume(VolumeSnapshot {
            level,
            muted: false,
            device_name: "Realtek".into(),
        })
    }

    fn shows(fx: &[Effect]) -> usize {
        fx.iter().filter(|e| **e == Effect::Show).count()
    }

    fn phases(fx: &[Effect]) -> Vec<Phase> {
        fx.iter()
            .filter_map(|e| match e {
                Effect::Emit(f) => Some(f.phase),
                _ => None,
            })
            .collect()
    }

    /// Açılışta gönderilen başlangıç anlık görüntüsü durumu doldurur ama kutuyu
    /// açmaz — ne `Show` ne de `Emit` üretir.
    #[test]
    fn acilis_anlik_goruntusu_sessizdir() {
        let mut e = Engine::default();
        let fx = e.handle(media("Telephones", PlaybackState::Paused), 0);
        assert!(fx.is_empty(), "{fx:?}");
        assert_eq!(e.state().media.as_ref().unwrap().track.title, "Telephones");
        assert!(!e.is_on_screen());
    }

    /// Çıkış kriteri 3: parça değişiminin tamamı (P1 § 6.3'teki 8 olaylık seri)
    /// tek `SW_SHOW` üretir, başlık 270. ms'de **yerinde** güncellenir.
    #[test]
    fn parca_gecisi_tek_kutu_acar_baslik_yerinde_guncellenir() {
        let mut e = Engine::default();
        e.handle(media("Telephones", PlaybackState::Playing), 0); // ilk olay yutulur

        let mut fx = Vec::new();
        // +67 ms: durum olayı, başlık henüz eski parçanın.
        fx.extend(e.handle(media("Telephones", PlaybackState::Paused), 67));
        // +270 ms: yeni başlık gelir. +271'de aynısı tekrar gelir (Spotify iki kez).
        fx.extend(e.handle(media("Anhedoni", PlaybackState::Paused), 270));
        fx.extend(e.handle(media("Anhedoni", PlaybackState::Paused), 271));

        assert_eq!(shows(&fx), 1, "kutu birden fazla kez açıldı: {fx:?}");
        assert_eq!(phases(&fx), vec![Phase::Enter, Phase::Update]);
        assert_eq!(e.state().media.as_ref().unwrap().track.title, "Anhedoni");
    }

    /// Çıkış kriteri 1: ses tuşuna 10 kez arka arkaya bas → tek kutu, tek açılış.
    #[test]
    fn on_ses_tusu_tek_kutu() {
        let mut e = Engine::default();
        e.handle(vol(0.50), 0); // açılış anlık görüntüsü

        let mut fx = Vec::new();
        for i in 1..=10u64 {
            fx.extend(e.handle(vol(0.50 + i as f32 * 0.02), i * 80));
        }
        assert_eq!(shows(&fx), 1, "{fx:?}");
        assert_eq!(
            phases(&fx).iter().filter(|p| **p == Phase::Enter).count(),
            1
        );
        // Son tuştan itibaren tam süre kalmalı.
        assert_eq!(e.next_timeout(800), Some(2_500));
    }

    /// Çıkış kriteri 4: tarayıcı sekmesi ↔ Spotify geçişinde boş kutu görünmüyor.
    #[test]
    fn bos_dogan_oturum_kutuyu_acmaz_ve_icerigi_silmez() {
        let mut e = Engine::default();
        e.handle(media("Telephones", PlaybackState::Playing), 0);

        let bos = DomainEvent::Media(MediaSnapshot {
            source_id: "MSEdge".into(),
            state: PlaybackState::Opened,
            ..Default::default()
        });
        let fx = e.handle(
            DomainEvent::SessionSwitched {
                source_id: "MSEdge".into(),
            },
            100,
        );
        assert!(fx.is_empty(), "{fx:?}");
        let fx = e.handle(bos, 100);
        assert!(fx.is_empty(), "boş oturum kutuyu açtı: {fx:?}");
        assert!(!e.is_on_screen());
        // Eski içerik silinmedi; kaynak kimliği tazelendi.
        let m = e.state().media.as_ref().unwrap();
        assert_eq!(m.track.title, "Telephones");
        assert_eq!(m.source_id, "MSEdge");

        // 14 ms sonra gerçek içerik gelir — kutu asıl o zaman açılır.
        let fx = e.handle(media("Test Parcasi A", PlaybackState::Playing), 114);
        assert_eq!(shows(&fx), 1, "{fx:?}");
    }

    #[test]
    fn sure_dolunca_once_fade_sonra_gercek_gizleme() {
        let mut e = Engine::default();
        e.handle(media("ilk", PlaybackState::Playing), 0);
        e.handle(media("Anhedoni", PlaybackState::Playing), 10);

        assert!(e.tick(2_000).is_empty());
        assert_eq!(phases(&e.tick(2_510)), vec![Phase::Leave]);
        assert_eq!(e.tick(2_660), vec![Effect::Hide]);
        assert!(!e.is_on_screen());
        assert_eq!(e.next_timeout(2_660), None);
    }

    /// Gizliyken WebView'e hiçbir şey gönderilmez.
    #[test]
    fn gizliyken_emit_yok() {
        let mut e = Engine::default();
        e.handle(media("ilk", PlaybackState::Playing), 0);
        // Tetik olmayan ama durumu değiştiren bir olay: oturum kapandı.
        let fx = e.handle(DomainEvent::SessionGone, 50);
        assert!(fx.is_empty(), "{fx:?}");
    }
}
