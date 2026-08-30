//! Domain tipleri — adapter'ların dışarıya verdiği tek dil.

use serde::Serialize;

/// SMTC'nin `...SessionPlaybackStatus`'ünün domain karşılığı. Çeviriyi
/// `adapters/smtc.rs` yapar; core WinRT enum'unu görmez.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PlaybackState {
    Closed,
    Opened,
    Changing,
    Stopped,
    Paused,
    Playing,
    Unknown,
}

impl PlaybackState {
    /// Kutuyu açmaya aday durumlar. `Opened` ve `Closed` **açmaz** — yeni oturum
    /// boş doğuyor ve içerik 14 ms sonra geliyor (`docs/P1-SONUC.md` § 5.1).
    /// `Changing` tolere edilir ama beklenmez (§ 6.4).
    pub fn is_showable(self) -> bool {
        matches!(
            self,
            Self::Playing | Self::Paused | Self::Stopped | Self::Changing
        )
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Track {
    pub title: String,
    pub artist: String,
    /// Tarayıcı oturumlarında **boş gelebilir** (`docs/P1-SONUC.md` § 3).
    pub album: String,
}

/// Pozisyon. `LastUpdatedTime == 0` olan oturumda pozisyon "bilinmiyor" sayılır ve
/// bu alan `None` olur — interpolasyona sokulmaz (`docs/P1-SONUC.md` § 5.2).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Position {
    pub position_ms: u64,
    pub duration_ms: u64,
    /// Ölçümün ne kadar bayat olduğu. P3'teki interpolasyonun girdisi.
    pub age_ms: u64,
}

/// Kaynağın desteklediği komutlar. Her kaynak seek/next desteklemez; P3 butonu
/// buna göre pasifleştirir (`docs/P1-SONUC.md` § 3).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct Controls {
    pub play: bool,
    pub pause: bool,
    pub toggle: bool,
    pub next: bool,
    pub prev: bool,
    pub stop: bool,
    pub seek: bool,
}

/// Bir SMTC olayının anlık görüntüsü. Adapter durum tutmadığı için her olay
/// kendi kendine yeter (`docs/P1-SONUC.md` § 7).
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaSnapshot {
    pub source_id: String,
    pub track: Track,
    pub state: PlaybackState,
    pub position: Option<Position>,
    pub controls: Controls,
    /// Kaynak uygulama şu an ön planda mı? 4. gürültü kuralının girdisi.
    /// Hesabı Windows'a bağlı olduğu için adapter dolduruyor.
    pub source_is_foreground: bool,
    /// Kapak görseli, `data:image/...;base64,...`. Adapter kutu **gizliyken**
    /// çözüp parça başına cache'liyor; ilk boyanan karede hazır olsun diye
    /// (`docs/ROADMAP.md` P3). Kaynak vermiyorsa `None` → `--art-fallback`.
    pub art: Option<String>,
}

impl Default for MediaSnapshot {
    fn default() -> Self {
        Self {
            source_id: String::new(),
            track: Track::default(),
            state: PlaybackState::Unknown,
            position: None,
            controls: Controls::default(),
            source_is_foreground: false,
            art: None,
        }
    }
}

impl MediaSnapshot {
    /// Gösterilecek bir şey var mı? Boş doğan oturumun kutuyu açmasını engelleyen
    /// asıl kontrol bu (`docs/P1-SONUC.md` § 5.1).
    pub fn has_content(&self) -> bool {
        !self.track.title.is_empty() || !self.track.artist.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VolumeSnapshot {
    /// 0.0 – 1.0
    pub level: f32,
    pub muted: bool,
    /// "Realtek · Hoparlör" — `docs/DESIGN.md` ses kutusundaki `.fly__source`.
    pub device_name: String,
}

impl Default for VolumeSnapshot {
    fn default() -> Self {
        Self {
            level: 0.0,
            muted: false,
            device_name: String::new(),
        }
    }
}

impl VolumeSnapshot {
    /// Yüzde olarak yuvarlanmış seviye. Gürültü kuralı bunun üzerinden karşılaştırır:
    /// bazı sürücüler 0.64 yerine 0.6400001 bildiriyor, float eşitliği güvenilmez.
    pub fn percent(&self) -> u8 {
        (self.level.clamp(0.0, 1.0) * 100.0).round() as u8
    }
}

/// Hangi kutu gösteriliyor. Ölçüler farklı: medya 400×120, ses 400×84.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum FlyoutKind {
    Media,
    Volume,
}

/// Tek doğruluk kaynağı. Tek yazıcı görevde tutulur, kopyası WebView'e gider.
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerState {
    pub media: Option<MediaSnapshot>,
    pub volume: Option<VolumeSnapshot>,
}
