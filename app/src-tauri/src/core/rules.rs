//! Gürültü kuralları — `docs/ARCHITECTURE.md` "Durum dinlemenin bedeli: gürültü",
//! P1 ölçümüyle düzeltilmiş hâliyle (`docs/P1-SONUC.md` § 6 ve § 8).
//!
//! Buradaki tek soru: **bu olay kutuyu açmalı mı?** Kutunun ne kadar duracağı,
//! animasyonun baştan oynayıp oynamayacağı `visibility.rs`'in işi. 3. kuralın
//! (birleştirme penceresi) gösterim kararını yönetmemesinin sebebi de bu:
//! tek parça geçişi 811 ms'ye yayılan 8 olay üretiyor ve başlık 270. ms'de
//! geliyor — beklenirse kutu geç açılır (§ 6.3).

use super::event::DomainEvent;
use super::model::{MediaSnapshot, PlaybackState, VolumeSnapshot};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// Kutuyu aç (ya da görünüyorsa süresini uzat).
    Trigger,
    Drop(DropReason),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DropReason {
    /// 1. kural — abonelik anında gelen ilk olay yutulur, yoksa uygulama her
    /// açılışta kendini gösterir. Bu makinede tetiklenmedi ama koruma amaçlı
    /// duruyor (§ 6.1); asıl işi başlangıç anlık görüntüsünü yutmak.
    FirstEvent,
    /// 2. kural — aynı (başlık, sanatçı, durum) üçlüsü. Spotify aynı olayı 2,
    /// Edge 3 kez gönderiyor (§ 6.2).
    Unchanged,
    /// `Opened` / `Closed`: kutuyu açmaz (§ 5.1).
    NotShowable,
    /// Oturum var ama içeriği yok — boş kutu görünmesin (§ 5.1).
    NoContent,
    /// 4. kural — kaynak uygulama ön planda (ayar, varsayılan kapalı).
    Foreground,
    /// `CurrentSessionChanged` / oturum yok: durum güncellenir, kutu açılmaz.
    SessionOnly,
}

#[derive(Clone, Copy, Debug)]
pub struct RuleConfig {
    /// 4. kural açık mı? Varsayılan **kapalı**: kaynak eşleştirmesi AUMID ↔ süreç
    /// adı üzerinden sezgisel yapılıyor, yanlış pozitifte kutu hiç çıkmaz.
    pub skip_when_foreground: bool,
}

impl Default for RuleConfig {
    fn default() -> Self {
        Self { skip_when_foreground: false }
    }
}

/// Kuralların hatırladığı tek şey "en son neyi tetik saydım". Oynatıcı durumu
/// burada tutulmaz — o `PlayerState`'in işi.
#[derive(Debug, Default)]
pub struct Gatekeeper {
    cfg: RuleConfig,
    seen_media: bool,
    seen_volume: bool,
    last_media_key: Option<(String, String, PlaybackState)>,
    last_volume_key: Option<(u8, bool)>,
}

impl Gatekeeper {
    pub fn new(cfg: RuleConfig) -> Self {
        Self { cfg, ..Default::default() }
    }

    pub fn judge(&mut self, ev: &DomainEvent) -> Verdict {
        match ev {
            DomainEvent::Media(s) => self.judge_media(s),
            DomainEvent::Volume(v) => self.judge_volume(v),
            DomainEvent::SessionSwitched { .. } | DomainEvent::SessionGone => {
                Verdict::Drop(DropReason::SessionOnly)
            }
        }
    }

    fn judge_media(&mut self, s: &MediaSnapshot) -> Verdict {
        let key = (s.track.title.clone(), s.track.artist.clone(), s.state);

        if !self.seen_media {
            self.seen_media = true;
            self.last_media_key = Some(key);
            return Verdict::Drop(DropReason::FirstEvent);
        }
        if !s.state.is_showable() {
            return Verdict::Drop(DropReason::NotShowable);
        }
        if !s.has_content() {
            return Verdict::Drop(DropReason::NoContent);
        }
        if self.last_media_key.as_ref() == Some(&key) {
            return Verdict::Drop(DropReason::Unchanged);
        }
        self.last_media_key = Some(key);
        if self.cfg.skip_when_foreground && s.source_is_foreground {
            return Verdict::Drop(DropReason::Foreground);
        }
        Verdict::Trigger
    }

    fn judge_volume(&mut self, v: &VolumeSnapshot) -> Verdict {
        let key = (v.percent(), v.muted);

        if !self.seen_volume {
            self.seen_volume = true;
            self.last_volume_key = Some(key);
            return Verdict::Drop(DropReason::FirstEvent);
        }
        if self.last_volume_key == Some(key) {
            return Verdict::Drop(DropReason::Unchanged);
        }
        self.last_volume_key = Some(key);
        Verdict::Trigger
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::model::Track;

    fn media(title: &str, artist: &str, state: PlaybackState) -> DomainEvent {
        DomainEvent::Media(MediaSnapshot {
            source_id: "test!App".into(),
            track: Track { title: title.into(), artist: artist.into(), album: String::new() },
            state,
            ..Default::default()
        })
    }

    fn vol(level: f32, muted: bool) -> DomainEvent {
        DomainEvent::Volume(VolumeSnapshot { level, muted, device_name: "Test".into() })
    }

    /// Uygulama açılışında gönderilen başlangıç anlık görüntüsü kutuyu açmamalı.
    #[test]
    fn ilk_olay_yutulur() {
        let mut g = Gatekeeper::default();
        assert_eq!(
            g.judge(&media("A", "B", PlaybackState::Playing)),
            Verdict::Drop(DropReason::FirstEvent)
        );
        assert_eq!(g.judge(&media("C", "D", PlaybackState::Playing)), Verdict::Trigger);
    }

    /// P1 § 6.2: Spotify aynı olayı 2, Edge 3 kez gönderdi.
    #[test]
    fn ayni_uclu_tekrar_gelirse_yutulur() {
        let mut g = Gatekeeper::default();
        g.judge(&media("ilk", "", PlaybackState::Playing));
        assert_eq!(g.judge(&media("A", "B", PlaybackState::Playing)), Verdict::Trigger);
        assert_eq!(
            g.judge(&media("A", "B", PlaybackState::Playing)),
            Verdict::Drop(DropReason::Unchanged)
        );
        assert_eq!(
            g.judge(&media("A", "B", PlaybackState::Playing)),
            Verdict::Drop(DropReason::Unchanged)
        );
        // Durum değişirse aynı parça olsa bile tetik: kullanıcı duraklattı.
        assert_eq!(g.judge(&media("A", "B", PlaybackState::Paused)), Verdict::Trigger);
    }

    /// P1 § 5.1: yeni oturum boş doğuyor, durumu `Opened`.
    #[test]
    fn bos_dogan_oturum_kutuyu_acmaz() {
        let mut g = Gatekeeper::default();
        g.judge(&media("ilk", "", PlaybackState::Playing));
        assert_eq!(
            g.judge(&media("", "", PlaybackState::Opened)),
            Verdict::Drop(DropReason::NotShowable)
        );
        assert_eq!(
            g.judge(&media("", "", PlaybackState::Playing)),
            Verdict::Drop(DropReason::NoContent)
        );
        assert_eq!(
            g.judge(&media("", "", PlaybackState::Closed)),
            Verdict::Drop(DropReason::NotShowable)
        );
    }

    #[test]
    fn oturum_degisimi_tek_basina_tetik_degil() {
        let mut g = Gatekeeper::default();
        g.judge(&media("ilk", "", PlaybackState::Playing));
        let ev = DomainEvent::SessionSwitched { source_id: "MSEdge".into() };
        assert_eq!(g.judge(&ev), Verdict::Drop(DropReason::SessionOnly));
        assert_eq!(g.judge(&DomainEvent::SessionGone), Verdict::Drop(DropReason::SessionOnly));
    }

    #[test]
    fn ses_seviyesi_ayni_yuzdede_yutulur() {
        let mut g = Gatekeeper::default();
        assert_eq!(g.judge(&vol(0.50, false)), Verdict::Drop(DropReason::FirstEvent));
        assert_eq!(g.judge(&vol(0.64, false)), Verdict::Trigger);
        // Sürücü kaynaklı float gürültüsü aynı yüzdeye yuvarlanıyor.
        assert_eq!(g.judge(&vol(0.640_000_1, false)), Verdict::Drop(DropReason::Unchanged));
        assert_eq!(g.judge(&vol(0.64, true)), Verdict::Trigger);
    }

    #[test]
    fn on_plandaki_ayar_kapaliyken_gecer_acikken_atlanir() {
        let fg = |on: bool| {
            DomainEvent::Media(MediaSnapshot {
                track: Track { title: "A".into(), artist: "B".into(), album: String::new() },
                state: PlaybackState::Playing,
                source_is_foreground: on,
                ..Default::default()
            })
        };

        let mut kapali = Gatekeeper::default();
        kapali.judge(&media("ilk", "", PlaybackState::Playing));
        assert_eq!(kapali.judge(&fg(true)), Verdict::Trigger);

        let mut acik = Gatekeeper::new(RuleConfig { skip_when_foreground: true });
        acik.judge(&media("ilk", "", PlaybackState::Playing));
        assert_eq!(acik.judge(&fg(true)), Verdict::Drop(DropReason::Foreground));
    }
}
