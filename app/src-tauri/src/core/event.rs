//! Adapter → Core yönündeki tek arayüz. Adapter'lar bu tipleri mpsc'ye yazar,
//! başka hiçbir şey paylaşmaz (`docs/P1-SONUC.md` § 7).

use super::model::{MediaSnapshot, VolumeSnapshot};

#[derive(Clone, Debug, PartialEq)]
pub enum DomainEvent {
    /// `MediaPropertiesChanged` veya `PlaybackInfoChanged`.
    /// `TimelinePropertiesChanged` **tetik değildir** ve buraya hiç gelmez
    /// (`docs/P1-SONUC.md` § 6.5) — pozisyon her anlık görüntüyle taze gelir.
    Media(MediaSnapshot),
    /// `CurrentSessionChanged`. **Tek başına kutuyu açmaz**; yeni oturum boş
    /// doğuyor, içerik 14 ms sonra `Media` ile geliyor (§ 5.1).
    SessionSwitched { source_id: String },
    /// SMTC'ye kayıtlı hiçbir oturum kalmadı.
    SessionGone,
    /// `IAudioEndpointVolumeCallback::OnNotify` — donanım tuşu, mixer, kulaklık.
    Volume(VolumeSnapshot),
}
