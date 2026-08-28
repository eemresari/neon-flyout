//! Görünürlük durum makinesi — `docs/ARCHITECTURE.md` § "Görünürlük durum makinesi".
//!
//! ```text
//! Hidden  ──(tetik)──────────► Visible   deadline = now + 2500 ms
//! Visible ──(yeni tetik)─────► Visible   deadline'ı ileri al, içeriği güncelle
//!                                        YENİ PENCERE AÇMA, ANİMASYONU BAŞTAN OYNATMA
//! Visible ──(deadline doldu)─► FadeOut   150 ms
//! FadeOut ──(fade bitti)─────► Hidden    SW_HIDE  ← opaklık sıfırlamak DEĞİL
//! ```
//!
//! Tek deadline, tek yazıcı. Flyout'ların bozuk hissetmesinin sebebi genelde üst
//! üste binen zamanlayıcılardır; burada zamanlayıcı yok — `next_timeout()` bir
//! sonraki uyanma anını söyler, sürücü döngü `recv_timeout` ile bekler.

/// Kutunun ekranda kalma süresi.
pub const SHOW_MS: u64 = 2_500;
/// Çıkış animasyonu. Bitiminde gerçek `SW_HIDE`.
pub const FADE_MS: u64 = 150;
/// Aynı "seri"nin süresi. P1 § 6.3: tek parça geçişi 811 ms'ye yayılan 8 olay
/// üretiyor. Bu pencere **gösterim kararını yönetmez** — sadece art arda gelen
/// olayların aynı kullanıcı eyleminden geldiğini işaretler.
pub const COALESCE_MS: u64 = 1_000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Visibility {
    Hidden,
    Visible,
    FadeOut,
}

/// Bir tetiğin görünürlük tarafındaki sonucu.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShowOutcome {
    /// Gizliden geldi: `SW_SHOW` + giriş animasyonu.
    Opened,
    /// Kaybolurken yakalandı: pencere zaten ekranda, opaklık geri alınır.
    Resumed,
    /// Görünürken geldi: sadece içerik ve deadline. Animasyon oynatılmaz.
    UpdatedInPlace,
}

/// Deadline dolduğunda makinenin ürettiği faz geçişi.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Expiry {
    /// Süre doldu, çıkış animasyonu başlasın.
    StartFadeOut,
    /// Animasyon bitti, pencere gerçekten gizlensin.
    Hide,
}

#[derive(Clone, Copy, Debug)]
pub struct VisibilityMachine {
    vis: Visibility,
    deadline_ms: u64,
    /// Kutunun en son gizliden açıldığı an. "Aynı seri mi?" bunun üzerinden.
    burst_start_ms: u64,
    show_ms: u64,
    fade_ms: u64,
    coalesce_ms: u64,
}

impl Default for VisibilityMachine {
    fn default() -> Self {
        Self::new(SHOW_MS, FADE_MS, COALESCE_MS)
    }
}

impl VisibilityMachine {
    pub fn new(show_ms: u64, fade_ms: u64, coalesce_ms: u64) -> Self {
        Self {
            vis: Visibility::Hidden,
            deadline_ms: 0,
            burst_start_ms: 0,
            show_ms,
            fade_ms,
            coalesce_ms,
        }
    }

    #[allow(dead_code)]
    pub fn visibility(&self) -> Visibility {
        self.vis
    }

    pub fn is_on_screen(&self) -> bool {
        !matches!(self.vis, Visibility::Hidden)
    }

    /// Tetik geldi. Dönen ikili: (ne yapıldı, bu olay önceki serinin devamı mı).
    pub fn trigger(&mut self, now_ms: u64) -> (ShowOutcome, bool) {
        let outcome = match self.vis {
            Visibility::Hidden => {
                self.burst_start_ms = now_ms;
                ShowOutcome::Opened
            }
            Visibility::FadeOut => ShowOutcome::Resumed,
            Visibility::Visible => ShowOutcome::UpdatedInPlace,
        };
        let same_burst = outcome != ShowOutcome::Opened
            && now_ms.saturating_sub(self.burst_start_ms) <= self.coalesce_ms;

        self.vis = Visibility::Visible;
        self.deadline_ms = now_ms + self.show_ms;
        (outcome, same_burst)
    }

    /// Deadline doldu mu? Sürücü döngü her uyanışta bir kez çağırır.
    pub fn tick(&mut self, now_ms: u64) -> Option<Expiry> {
        match self.vis {
            Visibility::Visible if now_ms >= self.deadline_ms => {
                self.vis = Visibility::FadeOut;
                self.deadline_ms = now_ms + self.fade_ms;
                Some(Expiry::StartFadeOut)
            }
            Visibility::FadeOut if now_ms >= self.deadline_ms => {
                self.vis = Visibility::Hidden;
                Some(Expiry::Hide)
            }
            _ => None,
        }
    }

    /// Bir sonraki uyanmaya kaç ms kaldı. `None` = uyanma gerekmiyor (gizli).
    pub fn next_timeout(&self, now_ms: u64) -> Option<u64> {
        match self.vis {
            Visibility::Hidden => None,
            _ => Some(self.deadline_ms.saturating_sub(now_ms)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn gizliden_acilir_ve_suresi_dolunca_kapanir() {
        let mut m = VisibilityMachine::default();
        assert_eq!(m.next_timeout(0), None);

        assert_eq!(m.trigger(0), (ShowOutcome::Opened, false));
        assert_eq!(m.visibility(), Visibility::Visible);
        assert_eq!(m.next_timeout(0), Some(2_500));

        assert_eq!(m.tick(2_499), None);
        assert_eq!(m.tick(2_500), Some(Expiry::StartFadeOut));
        assert_eq!(m.visibility(), Visibility::FadeOut);

        assert_eq!(m.tick(2_600), None);
        assert_eq!(m.tick(2_650), Some(Expiry::Hide));
        assert_eq!(m.visibility(), Visibility::Hidden);
        assert_eq!(m.next_timeout(2_650), None);
    }

    /// Çıkış kriteri 1: ses tuşuna 10 kez arka arkaya bas → tek kutu.
    #[test]
    fn on_ard_arda_tetik_tek_kutu_acar() {
        let mut m = VisibilityMachine::default();
        let mut acilis = 0;
        for i in 0..10u64 {
            let (outcome, _) = m.trigger(i * 80);
            if outcome == ShowOutcome::Opened {
                acilis += 1;
            }
        }
        assert_eq!(acilis, 1, "kutu birden fazla kez açıldı");
        // Son tetikten itibaren tam süre kalmalı — deadline ileri alındı.
        assert_eq!(m.next_timeout(720), Some(2_500));
    }

    /// P1 § 6.3'teki seri: 8 olay, 811 ms. Kutu bir kez açılır, gerisi yerinde.
    #[test]
    fn parca_gecisi_serisi_animasyonu_bir_kez_oynatir() {
        let mut m = VisibilityMachine::default();
        let seri = [67u64, 67, 213, 270, 271, 271, 340, 811];
        let mut sonuclar = Vec::new();
        for t in seri {
            sonuclar.push(m.trigger(t));
        }
        assert_eq!(sonuclar[0], (ShowOutcome::Opened, false));
        assert!(
            sonuclar[1..].iter().all(|(o, burst)| *o == ShowOutcome::UpdatedInPlace && *burst),
            "seri içindeki olaylar yerinde güncellemeli ve aynı seriden sayılmalı: {sonuclar:?}"
        );
    }

    #[test]
    fn kaybolurken_gelen_tetik_pencereyi_geri_alir() {
        let mut m = VisibilityMachine::default();
        m.trigger(0);
        assert_eq!(m.tick(2_500), Some(Expiry::StartFadeOut));

        let (outcome, same_burst) = m.trigger(2_550);
        assert_eq!(outcome, ShowOutcome::Resumed);
        assert!(!same_burst, "2.5 sn sonra gelen olay yeni bir seridir");
        assert_eq!(m.visibility(), Visibility::Visible);
        assert_eq!(m.next_timeout(2_550), Some(2_500));
    }
}
