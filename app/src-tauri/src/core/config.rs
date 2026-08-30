//! Ayar verisi. **Windows tipi yok, dosya sistemi yok** — okuma/yazma
//! `adapters/config_file.rs`'in işi, burası yalnızca veri ve kural.
//!
//! Varsayılanlar burada yeniden yazılmaz: `visibility` ve `rules` sabitlerinden
//! okunur, yoksa ikinci bir gerçek kaynak doğar ve ikisi sessizce ayrışır.

use serde::{Deserialize, Serialize};

use super::engine::EngineConfig;
use super::rules::RuleConfig;
use super::visibility::{COALESCE_MS, FADE_MS, SHOW_MS};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Theme {
    #[default]
    Cyber,
    Neon,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Corner {
    #[default]
    TopRight,
    TopLeft,
    BottomRight,
    BottomLeft,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MonitorPick {
    #[default]
    Primary,
    Cursor,
}

/// Kırpma aralıkları. Kutu 0 ms görünmemeli, bir dakika da durmamalı.
const SHOW_RANGE: (u64, u64) = (500, 15_000);
const FADE_RANGE: (u64, u64) = (0, 2_000);
const COALESCE_RANGE: (u64, u64) = (0, 5_000);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    #[serde(deserialize_with = "esnek")]
    pub theme: Theme,
    #[serde(deserialize_with = "esnek")]
    pub corner: Corner,
    #[serde(deserialize_with = "esnek")]
    pub monitor: MonitorPick,
    pub show_ms: u64,
    pub fade_ms: u64,
    pub coalesce_ms: u64,
    pub skip_when_foreground: bool,
    /// Bilgi amaçlı yazılır; **gerçek kaynak Başlangıç klasöründeki kısayoldur**
    /// (`adapters/autostart.rs`). Okunduğunda dosya durumu kazanır.
    pub autostart: bool,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            theme: Theme::default(),
            corner: Corner::default(),
            monitor: MonitorPick::default(),
            show_ms: SHOW_MS,
            fade_ms: FADE_MS,
            coalesce_ms: COALESCE_MS,
            skip_when_foreground: RuleConfig::default().skip_when_foreground,
            autostart: false,
        }
    }
}

impl Config {
    /// Bozuk JSON çökertmez: uyarı basar, varsayılanı döner. **Dosyanın üstüne
    /// yazmak arayanın işi değil** — çağıran taraf bozuk dosyayı korur.
    pub fn from_json_str(s: &str) -> Self {
        let mut cfg = match serde_json::from_str::<Self>(s) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("[config] okunamadi ({e}); varsayilanlar kullaniliyor");
                Self::default()
            }
        };
        cfg.sanitize();
        cfg
    }

    pub fn to_json_string(&self) -> String {
        // `unwrap` güvenli: `Config` yalnızca skaler ve alan-adlı enum içeriyor.
        serde_json::to_string_pretty(self).unwrap()
    }

    pub fn sanitize(&mut self) {
        let kis = |v: u64, (lo, hi): (u64, u64)| v.clamp(lo, hi);
        self.show_ms = kis(self.show_ms, SHOW_RANGE);
        self.fade_ms = kis(self.fade_ms, FADE_RANGE);
        self.coalesce_ms = kis(self.coalesce_ms, COALESCE_RANGE);
    }

    pub fn engine(&self) -> EngineConfig {
        EngineConfig {
            show_ms: self.show_ms,
            fade_ms: self.fade_ms,
            coalesce_ms: self.coalesce_ms,
            rules: RuleConfig {
                skip_when_foreground: self.skip_when_foreground,
            },
        }
    }
}

/// Tanınmayan bir enum değeri (`"theme": "mor"`) yalnızca **o alanı** varsayılana
/// düşürsün; tüm dosyayı kaybettirmesin. Kullanıcı bu dosyayı elle düzenliyor,
/// bir harf yanlış yazınca bütün ayarını kaybetmemeli.
///
/// Önce `Value`'ya alınıyor: `T::deserialize(d)` doğrudan çağrılsa deserializer
/// tüketilir ve hata durumunda `serde_json` akışı bozulur.
fn esnek<'de, D, T>(d: D) -> Result<T, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::de::DeserializeOwned + Default,
{
    let v = serde_json::Value::deserialize(d)?;
    Ok(serde_json::from_value(v).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bos_json_varsayilani_verir() {
        let c = Config::from_json_str("{}");
        assert_eq!(c, Config::default());
    }

    #[test]
    fn eksik_alan_varsayilana_duser() {
        let c = Config::from_json_str(r#"{"theme":"neon"}"#);
        assert_eq!(c.theme, Theme::Neon);
        assert_eq!(c.corner, Config::default().corner);
        assert_eq!(c.show_ms, Config::default().show_ms);
    }

    #[test]
    fn taninmayan_alan_yok_sayilir() {
        let c = Config::from_json_str(r#"{"theme":"neon","gelecekteki_ayar":42}"#);
        assert_eq!(c.theme, Theme::Neon);
    }

    #[test]
    fn bozuk_json_tamamen_varsayilan_verir() {
        // Kullanicinin elle bozdugu dosya: cokmek yok, varsayilanla devam.
        let c = Config::from_json_str("{ bu json degil ");
        assert_eq!(c, Config::default());
    }

    #[test]
    fn taninmayan_enum_degeri_o_alani_varsayilana_duser() {
        // "mor" diye tema yok; dosyanin geri kalani yine de okunmali.
        let c = Config::from_json_str(r#"{"theme":"mor","corner":"bottom-left"}"#);
        assert_eq!(c.theme, Config::default().theme);
        assert_eq!(c.corner, Corner::BottomLeft);
    }

    #[test]
    fn aralik_disi_sayilar_kirpilir() {
        let c = Config::from_json_str(r#"{"show_ms":999999,"fade_ms":99999,"coalesce_ms":99999}"#);
        assert_eq!(c.show_ms, 15_000);
        assert_eq!(c.fade_ms, 2_000);
        assert_eq!(c.coalesce_ms, 5_000);

        let c = Config::from_json_str(r#"{"show_ms":0}"#);
        assert_eq!(c.show_ms, 500);
    }

    #[test]
    fn engine_config_ayardan_turetilir() {
        let c = Config::from_json_str(r#"{"show_ms":4000,"skip_when_foreground":true}"#);
        let e = c.engine();
        assert_eq!(e.show_ms, 4_000);
        assert!(e.rules.skip_when_foreground);
    }

    #[test]
    fn varsayilanlar_core_sabitleriyle_ayni() {
        // Ayar dosyasi varsayilanlari ikinci bir gercek kaynak olmamali.
        let d = Config::default();
        assert_eq!(d.show_ms, SHOW_MS);
        assert_eq!(d.fade_ms, FADE_MS);
        assert_eq!(d.coalesce_ms, COALESCE_MS);
        assert_eq!(
            d.skip_when_foreground,
            RuleConfig::default().skip_when_foreground
        );
    }

    #[test]
    fn gidis_donus_kayipsiz() {
        let c = Config {
            theme: Theme::Neon,
            corner: Corner::BottomLeft,
            monitor: MonitorPick::Cursor,
            ..Config::default()
        };
        assert_eq!(Config::from_json_str(&c.to_json_string()), c);
    }
}
