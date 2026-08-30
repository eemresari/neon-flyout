# P5 — Paketleme: uygulama planı

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** neon-flyout'u kurulup unutulan bir tepsi uygulamasına çevirmek: ayar dosyası, tepsi menüsü, Windows ile başlat, NSIS installer.

**Architecture:** Ayar tek bir küresel nesneden okunmaz; **tüketicisine göre bölünür**. Tema WebView'e emit edilir, köşe/monitör `adapters/window.rs`'teki bir statiğe yazılır, zamanlama ve kural ayarları mevcut mpsc kanalından `DomainEvent::ConfigChanged` olarak core döngüsüne gider — böylece `Engine`'e yalnızca core thread'inin dokunduğu kural (P2'nin tek-yazıcı disiplini) bozulmaz. Yetkili `Config` nesnesi Tauri app state'inde bir `Mutex<Config>`'tur; **oradan okunmaz, oradan itilir** — tepsi ve dosya yükleme bu nesneyi değiştirip üç tüketiciye ayrı ayrı yollar.

**Tech Stack:** Rust 1.98 · Tauri 2.11.5 (`tray-icon` feature) · `windows` crate 0.61 (`Win32_UI_Shell` eklenecek) · serde/serde_json · vanilla TS + Vite 6 · NSIS bundler

**Spec:** `docs/superpowers/specs/2026-08-30-p5-paketleme-design.md`

## Global Constraints

- **Mimari kuralı:** `unsafe`, WinRT tipi ve COM arayüzü **yalnızca** `app/src-tauri/src/adapters/` altında görünür. `core/` Windows'a dair hiçbir tip bilmez. (`docs/ARCHITECTURE.md`)
- **`Engine`'e yalnızca core thread'i dokunur.** Engine'i başka bir thread'den değiştiren kod yazma; her değişiklik mpsc üzerinden geçer.
- **Tasarım değerleri `design/` altındadır.** Renk/boşluk/yazı boyutu koda gömülmez, yuvarlanmaz (`docs/DESIGN.md`).
- **Ayar dosyası yolu:** `%APPDATA%\neon-flyout\config.json`. `app_config_dir()` **kullanılmayacak** (bundle identifier'lı okunaksız yol verir).
- **Bozuk `config.json` üstüne yazılmaz.** Varsayılanla çalışılır, `stderr`'e tek satır uyarı yazılır, çökülmez.
- **Autostart'ın gerçek kaynağı Başlangıç klasöründeki `.lnk` dosyasıdır**, `config.json` değil.
- **Test komutu:** `cd app/src-tauri && cargo test`. Mevcut taban **16 test**; her task sonunda bu sayı azalmamalı.
- **Çalıştırma:** `cd app && npm run tauri dev`. `cargo run` tek başına çalışmaz.
- **PATH tuzağı:** `cargo`/`npm` "bulunamadı" derse komutun başına: `$env:Path = [Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [Environment]::GetEnvironmentVariable("Path","User")`
- **Dil:** kod yorumları ve kullanıcıya görünen metinler Türkçe; mevcut dosyaların üslubuna uy.

---

## Dosya haritası

| Dosya | Durum | Sorumluluk |
|---|---|---|
| `app/src-tauri/src/core/config.rs` | **yeni** | `Config` verisi: serde, varsayılan, aralık kırpma, `EngineConfig`'e çevirme. Windows tipi yok, birim testli. |
| `app/src-tauri/src/core/place.rs` | **yeni** | Saf geometri: çalışma alanı + pencere boyutu + köşe → (x, y). Birim testli. |
| `app/src-tauri/src/core/engine.rs` | değişiyor | `Engine::reconfigure`; `EngineConfig`/`RuleConfig` derive'ları |
| `app/src-tauri/src/core/visibility.rs` | değişiyor | `VisibilityMachine::reconfigure` |
| `app/src-tauri/src/core/rules.rs` | değişiyor | `Gatekeeper::reconfigure`; `ConfigChanged` için `DropReason::Config` |
| `app/src-tauri/src/core/event.rs` | değişiyor | `DomainEvent::ConfigChanged(EngineConfig)` |
| `app/src-tauri/src/adapters/config_file.rs` | **yeni** | Yol, oku, yaz, dosyayı gezginde aç |
| `app/src-tauri/src/adapters/autostart.rs` | **yeni** | COM `IShellLinkW` ile `.lnk` yaz/sil/var mı |
| `app/src-tauri/src/adapters/tray.rs` | **yeni** | Tepsi ikonu, menü, olay yönlendirme |
| `app/src-tauri/src/adapters/window.rs` | değişiyor | `set_placement`; `place_top_right` → `place` |
| `app/src-tauri/src/adapters/native_flyout.rs` | değişiyor | Kaydedilen ex-style'da bastırma bitlerini maskele |
| `app/src-tauri/src/lib.rs` | değişiyor | Config'i yükle, state'e koy, tepsi ve `ayarlar` komutunu bağla |
| `app/src-tauri/Cargo.toml` | değişiyor | `tray-icon` feature, `Win32_UI_Shell` feature |
| `app/src-tauri/tauri.conf.json` | değişiyor | `bundle.active`, NSIS, `webviewInstallMode` |
| `app/src/main.ts` | değişiyor | `ayarlar` komutunu çağır, `config` kanalını dinle |

---

## Task 1: Frontend varlık kapısı — `npm run build` fontları taşıyor mu?

Bu bir **ölçüm**, kod yazma işi değil. Installer'ın anlamı buna bağlı: `src/main.ts` `../../design/*.css` import ediyor, o CSS'ler `design/fonts/*.woff2`'ye `@font-face` ile bağlı ve `design/` Vite kökünün **dışında**. `vite.config.ts`'teki `server.fs.allow` yalnızca **dev sunucusu** için geçerli — build yolunu kanıtlamaz. Taşımıyorsa kutu kurulumda fontsuz çıkar ve bunu ancak installer'ı kurduktan sonra fark ederiz.

**Files:**
- Modify (yalnızca gerekirse): `app/vite.config.ts`

**Interfaces:**
- Consumes: yok
- Produces: sonraki taskların hepsi `npm run build`'in çalıştığını varsayar

- [ ] **Step 1: Temiz build al**

```bash
cd app && rm -rf dist && npm run build
```

Beklenen: hata yok, `dist/` oluşuyor. `tsc` hatası verirse **durma, kaydet** — TypeScript hatası Task 6'da kod eklerken de karşına çıkacak; şimdi düzelt.

- [ ] **Step 2: woff2'lerin `dist/`e taşındığını doğrula**

```bash
cd app && find dist -name '*.woff2' | wc -l && find dist -name '*.css' -exec grep -l 'woff2' {} \;
```

Beklenen: **10 adet** woff2 (`design/fonts/` altındaki sayı) ve en az bir CSS dosyası bunlara referans veriyor.

- [ ] **Step 3: CSS'teki font yollarının `dist/` içinde çözüldüğünü doğrula**

```bash
cd app && grep -o 'url([^)]*woff2[^)]*)' dist/assets/*.css | head -5
```

Beklenen: yollar `/assets/...woff2` gibi **dist içine** işaret ediyor. `../../design/fonts/...` gibi depo yoluna işaret ediyorsa **kapı kapalı** → Step 4.

- [ ] **Step 4: Kapı kapalıysa düzelt (yalnızca Step 2/3 başarısızsa)**

Vite CSS `url()`'lerini normalde otomatik toplar. Toplamadıysa sebep neredeyse kesinlikle kök dışı yol; çare `vite.config.ts`'e alias eklemek:

```ts
import { defineConfig } from "vite";
import { fileURLToPath, URL } from "node:url";

export default defineConfig(async () => ({
  resolve: {
    alias: {
      // `design/` depo kökünde, Vite kökünün (app/) dışında. Alias build
      // yolunda da çözülüyor; `server.fs.allow` yalnızca dev sunucusu için.
      "@design": fileURLToPath(new URL("../design", import.meta.url)),
    },
  },
  // ...mevcut clearScreen / server bloğu aynen kalır...
}));
```

ve `src/main.ts`'teki dört import'u `@design/...` biçimine çevir:

```ts
import "@design/fonts.css";
import "@design/flyout.css";
import "@design/themes/cyber.css";
import "@design/themes/neon.css";
```

Sonra Step 1–3'ü tekrarla.

- [ ] **Step 5: Sonucu belgele ve gerekiyorsa commit et**

Kapı **açıksa** (düzeltme gerekmediyse) commit yok; sonucu Task 10'un notlarına taşımak üzere aklında tut. Düzeltme gerektiyse:

```bash
git add app/vite.config.ts app/src/main.ts
git commit -m "P5: design/ varliklari build yolunda da cozuluyor (alias)"
```

---

## Task 2: `core/config.rs` — ayar verisi (TDD)

Saf veri katmanı: serde, varsayılanlar, aralık kırpma, `EngineConfig`'e çevirme. Windows tipi yok, dosya sistemi yok — bu yüzden tamamen birim testlenebilir ve P5'in tek gerçek test yükü burada.

**Files:**
- Create: `app/src-tauri/src/core/config.rs`
- Modify: `app/src-tauri/src/core/mod.rs`
- Modify: `app/src-tauri/src/core/engine.rs` (yalnızca derive satırları)
- Modify: `app/src-tauri/src/core/rules.rs` (yalnızca derive satırı)

**Interfaces:**
- Consumes: `core::engine::{EngineConfig}`, `core::rules::RuleConfig`, `core::visibility::{SHOW_MS, FADE_MS, COALESCE_MS}`
- Produces:
  - `pub enum Theme { Cyber, Neon }` (serde: `"cyber"`, `"neon"`)
  - `pub enum Corner { TopRight, TopLeft, BottomRight, BottomLeft }` (serde: `"top-right"` …)
  - `pub enum MonitorPick { Primary, Cursor }` (serde: `"primary"`, `"cursor"`)
  - `pub struct Config { theme, corner, monitor, show_ms, fade_ms, coalesce_ms, skip_when_foreground, autostart }` — hepsi `pub`
  - `impl Config { pub fn from_json_str(s: &str) -> Config; pub fn to_json_string(&self) -> String; pub fn sanitize(&mut self); pub fn engine(&self) -> EngineConfig }`

- [ ] **Step 1: `EngineConfig` ve `RuleConfig`'e gereken derive'ları ekle**

`DomainEvent` Task 4'te `EngineConfig` taşıyacak ve `DomainEvent` zaten `Clone, Debug, PartialEq`. Şimdiden hazırla.

`app/src-tauri/src/core/engine.rs` — `pub struct EngineConfig` satırının üstü:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EngineConfig {
```

`app/src-tauri/src/core/rules.rs` — `pub struct RuleConfig` satırının üstü:

```rust
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RuleConfig {
```

(`RuleConfig` zaten elle `Default` uyguluyor; `derive(Default)` **ekleme**, çakışır.)

- [ ] **Step 2: Başarısız testleri yaz**

`app/src-tauri/src/core/config.rs` dosyasını **yalnızca test modülüyle** oluştur:

```rust
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
        let c = Config::from_json_str(
            r#"{"show_ms":999999,"fade_ms":99999,"coalesce_ms":99999}"#,
        );
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
        assert_eq!(d.show_ms, crate::core::visibility::SHOW_MS);
        assert_eq!(d.fade_ms, crate::core::visibility::FADE_MS);
        assert_eq!(d.coalesce_ms, crate::core::visibility::COALESCE_MS);
        assert_eq!(
            d.skip_when_foreground,
            crate::core::rules::RuleConfig::default().skip_when_foreground
        );
    }

    #[test]
    fn gidis_donus_kayipsiz() {
        let mut c = Config::default();
        c.theme = Theme::Neon;
        c.corner = Corner::BottomLeft;
        c.monitor = MonitorPick::Cursor;
        assert_eq!(Config::from_json_str(&c.to_json_string()), c);
    }
}
```

`core/mod.rs`'e modülü ekle:

```rust
pub mod config;
```

ve `pub use` satırlarının altına:

```rust
pub use config::{Config, Corner, MonitorPick, Theme};
```

- [ ] **Step 3: Testlerin başarısız olduğunu gör**

```bash
cd app/src-tauri && cargo test config
```

Beklenen: **derleme hatası** — `Config`, `Theme`, `Corner`, `MonitorPick` tanımlı değil.

- [ ] **Step 4: Uygulamayı yaz**

`app/src-tauri/src/core/config.rs`'in **başına**, test modülünün üstüne:

```rust
//! Ayar verisi. **Windows tipi yok, dosya sistemi yok** — okuma/yazma
//! `adapters/config_file.rs`'in işi, burası yalnızca veri ve kural.
//!
//! Varsayılanlar burada yeniden yazılmaz: `visibility` ve `rules`
//! sabitlerinden okunur, yoksa ikinci bir gerçek kaynak doğar.

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

/// Kırpma aralıkları. Kutu 0 ms görünmemeli, 60 saniye de durmamalı.
const SHOW_RANGE: (u64, u64) = (500, 15_000);
const FADE_RANGE: (u64, u64) = (0, 2_000);
const COALESCE_RANGE: (u64, u64) = (0, 5_000);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub theme: Theme,
    pub corner: Corner,
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
        // `unwrap` güvenli: `Config` yalnızca skaler ve enum içeriyor.
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
```

**Dikkat — enum alanları tek başına `#[serde(default)]` ile kurtulmaz.** Serde tanınmayan bir enum değerinde (`"theme": "mor"`) **tüm nesnenin** ayrıştırmasını hataya düşürür; kullanıcı bir harf yanlış yazınca bütün ayarını kaybeder. Bu yüzden üç enum alanının her birine alan-bazlı kurtarma konuyor:

```rust
    #[serde(deserialize_with = "esnek")]
    pub theme: Theme,
    #[serde(deserialize_with = "esnek")]
    pub corner: Corner,
    #[serde(deserialize_with = "esnek")]
    pub monitor: MonitorPick,
```

ve dosyanın sonuna, test modülünün üstüne:

```rust
/// Tanınmayan bir enum değeri (`"theme": "mor"`) yalnızca **o alanı** varsayılana
/// düşürsün; tüm dosyayı kaybettirmesin. Kullanıcı bu dosyayı elle düzenliyor.
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
```

`use serde::Deserialize;` zaten dosyanın başında.

- [ ] **Step 5: Testlerin geçtiğini gör**

```bash
cd app/src-tauri && cargo test
```

Beklenen: **9 yeni test geçiyor**, mevcut 16 test hâlâ geçiyor → toplam **25**.

- [ ] **Step 6: Commit**

```bash
git add app/src-tauri/src/core/config.rs app/src-tauri/src/core/mod.rs app/src-tauri/src/core/engine.rs app/src-tauri/src/core/rules.rs
git commit -m "P5: core/config.rs — ayar verisi, varsayilan ve kirpma (testli)"
```

---

## Task 3: `core/place.rs` + `window::set_placement` — köşe ve monitör

Köşe hesabı saf aritmetik; Windows'a hiç ihtiyacı yok. Bu yüzden hesap `core/place.rs`'te testli duruyor, `adapters/window.rs` yalnızca `MONITORINFO`'yu ona veriyor. `show()` zaten her gösterimde konumlandırma yapıyor — köşe değişikliği bu sayede ek koda gerek kalmadan bir sonraki kutuda geçerli olur.

**Files:**
- Create: `app/src-tauri/src/core/place.rs`
- Modify: `app/src-tauri/src/core/mod.rs`
- Modify: `app/src-tauri/src/adapters/window.rs`

**Interfaces:**
- Consumes: `core::config::{Corner, MonitorPick}` (Task 2)
- Produces:
  - `core::place::WorkArea { left, top, right, bottom }` (i32)
  - `core::place::position(work: WorkArea, w: i32, h: i32, corner: Corner, margin: i32) -> (i32, i32)`
  - `adapters::window::set_placement(corner: Corner, monitor: MonitorPick)`
  - `adapters::window::place(raw: isize)` — `place_top_right`'ın yerini alır

- [ ] **Step 1: Başarısız testleri yaz**

`app/src-tauri/src/core/place.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::config::Corner;

    /// 1920×1080 ekran, görev çubuğu altta 40 px: çalışma alanı 0,0–1920,1040.
    const WORK: WorkArea = WorkArea { left: 0, top: 0, right: 1920, bottom: 1040 };

    #[test]
    fn sag_ust() {
        assert_eq!(position(WORK, 400, 120, Corner::TopRight, 16), (1504, 16));
    }

    #[test]
    fn sol_ust() {
        assert_eq!(position(WORK, 400, 120, Corner::TopLeft, 16), (16, 16));
    }

    #[test]
    fn sag_alt() {
        assert_eq!(position(WORK, 400, 120, Corner::BottomRight, 16), (1504, 904));
    }

    #[test]
    fn sol_alt() {
        assert_eq!(position(WORK, 400, 120, Corner::BottomLeft, 16), (16, 904));
    }

    #[test]
    fn ikincil_monitorun_negatif_koordinatlari_korunur() {
        // Soldaki ikinci ekran: koordinatlar negatif. Boşluk yine kenardan.
        let work = WorkArea { left: -1920, top: 0, right: 0, bottom: 1040 };
        assert_eq!(position(work, 400, 120, Corner::TopRight, 16), (-416, 16));
    }

    #[test]
    fn kutu_calisma_alanindan_genisse_tasmaz() {
        // Patolojik durum: pencere ekrandan geniş. Sol/üst kenara yapış.
        let work = WorkArea { left: 0, top: 0, right: 300, bottom: 100 };
        assert_eq!(position(work, 400, 120, Corner::TopRight, 16), (0, 0));
    }
}
```

`core/mod.rs`'e ekle:

```rust
pub mod place;
```

- [ ] **Step 2: Testlerin başarısız olduğunu gör**

```bash
cd app/src-tauri && cargo test place
```

Beklenen: derleme hatası — `WorkArea`, `position` tanımlı değil.

- [ ] **Step 3: Uygulamayı yaz**

`app/src-tauri/src/core/place.rs`'in başına:

```rust
//! Kutunun ekrandaki konumu — **saf aritmetik**. Windows tipi yok; `MONITORINFO`
//! okumak `adapters/window.rs`'in işi, hesap burada testlenebilir duruyor.

use super::config::Corner;

/// Monitörün görev çubuğu düşülmüş çalışma alanı, fiziksel piksel.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WorkArea {
    pub left: i32,
    pub top: i32,
    pub right: i32,
    pub bottom: i32,
}

/// Pencerenin sol-üst köşesi nereye konacak.
///
/// Kutu çalışma alanından büyükse boşluk uygulanmaz ve sol/üst kenara yapışır —
/// ekran dışına taşıp görünmez olmasındansa kırpılması yeğdir.
pub fn position(work: WorkArea, w: i32, h: i32, corner: Corner, margin: i32) -> (i32, i32) {
    let sag = (work.right - w - margin).max(work.left);
    let alt = (work.bottom - h - margin).max(work.top);
    let sol = work.left + margin;
    let ust = work.top + margin;

    match corner {
        Corner::TopRight => (sag, ust.min(alt)),
        Corner::TopLeft => (sol.min(sag), ust.min(alt)),
        Corner::BottomRight => (sag, alt),
        Corner::BottomLeft => (sol.min(sag), alt),
    }
}
```

- [ ] **Step 4: Testlerin geçtiğini gör**

```bash
cd app/src-tauri && cargo test place
```

Beklenen: **6 test PASS**. `kutu_calisma_alanindan_genisse_tasmaz` `(0, 0)` bekliyor: `sag = (300-400-16).max(0) = 0`, `ust.min(alt) = 16.min((100-120-16).max(0)=0) = 0`. ✔

- [ ] **Step 5: `window.rs`'i köşe/monitör alacak hâle getir**

`app/src-tauri/src/adapters/window.rs`'te `place_top_right` fonksiyonunu **sil** ve yerine şunu koy. `use` bloğuna eklenecekler: `MONITOR_DEFAULTTONEAREST`, `MonitorFromPoint`, `POINT`, `GetCursorPos`.

```rust
use std::sync::atomic::{AtomicU8, Ordering};

use crate::core::config::{Corner, MonitorPick};
use crate::core::place::{position, WorkArea};

/// Köşe ve monitör tercihi. Atomikte tutuluyor: tepsi menüsü (ana thread) yazar,
/// `show()` (yine ana thread, ama farklı bir çağrı yolundan) okur. Kilit gerekmiyor,
/// iki değer de tek baytlık ve birbirinden bağımsız.
static CORNER: AtomicU8 = AtomicU8::new(0);
static MONITOR: AtomicU8 = AtomicU8::new(0);

fn corner_of(v: u8) -> Corner {
    match v {
        1 => Corner::TopLeft,
        2 => Corner::BottomRight,
        3 => Corner::BottomLeft,
        _ => Corner::TopRight,
    }
}

fn corner_num(c: Corner) -> u8 {
    match c {
        Corner::TopRight => 0,
        Corner::TopLeft => 1,
        Corner::BottomRight => 2,
        Corner::BottomLeft => 3,
    }
}

/// Tepsi menüsünden çağrılır. Etkisi **bir sonraki gösterimde** görünür —
/// `show()` her seferinde yeniden konumlandırıyor, ekrandaki kutu zıplamasın.
pub fn set_placement(corner: Corner, monitor: MonitorPick) {
    CORNER.store(corner_num(corner), Ordering::Relaxed);
    MONITOR.store(matches!(monitor, MonitorPick::Cursor) as u8, Ordering::Relaxed);
}

/// Pencereyi seçili köşeye taşır. Ölçüler fiziksel pikselde okunuyor, bu yüzden
/// DPI ölçeklemesi ayrıca hesaplanmıyor.
pub fn place(raw: isize) {
    unsafe {
        let mut r = RECT::default();
        if GetWindowRect(hwnd(raw), &mut r).is_err() {
            return;
        }
        let (w, h) = (r.right - r.left, r.bottom - r.top);

        // `cursor` seçiliyse imlecin bulunduğu ekran; değilse birincil.
        let mon = if MONITOR.load(Ordering::Relaxed) == 1 {
            let mut p = POINT::default();
            if GetCursorPos(&mut p).is_ok() {
                MonitorFromPoint(p, MONITOR_DEFAULTTONEAREST)
            } else {
                MonitorFromWindow(hwnd(raw), MONITOR_DEFAULTTOPRIMARY)
            }
        } else {
            MonitorFromWindow(hwnd(raw), MONITOR_DEFAULTTOPRIMARY)
        };

        let mut mi = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(mon, &mut mi).as_bool() {
            return;
        }
        let work = WorkArea {
            left: mi.rcWork.left,
            top: mi.rcWork.top,
            right: mi.rcWork.right,
            bottom: mi.rcWork.bottom,
        };

        let (x, y) = position(work, w, h, corner_of(CORNER.load(Ordering::Relaxed)), MARGIN);

        let _ = SetWindowPos(
            hwnd(raw),
            Some(HWND_TOPMOST),
            x,
            y,
            0,
            0,
            SWP_NOSIZE | SWP_NOACTIVATE,
        );
    }
}
```

`MARGIN` sabitinin yorumunu güncelle (artık "köşe seçimi P3" değil):

```rust
/// Ekran kenarına bırakılan boşluk (fiziksel piksel). Köşe seçimi `core/place.rs`.
const MARGIN: i32 = 16;
```

`show()` içindeki `place_top_right(raw);` çağrısını `place(raw);` yap. `lib.rs`'teki `adapters::window::place_top_right(hwnd);` çağrısını `adapters::window::place(hwnd);` yap.

**Not — birincil monitör kaybı:** `MonitorFromWindow(..., MONITOR_DEFAULTTOPRIMARY)` pencere hangi ekranda olursa olsun birincil ekranı döner; `primary` seçeneğinin davranışı budur ve doğrudur.

- [ ] **Step 6: Derle ve testleri koştur**

```bash
cd app/src-tauri && cargo test
```

Beklenen: derleme temiz, **31 test** geçiyor (16 + 9 + 6).

- [ ] **Step 7: Commit**

```bash
git add app/src-tauri/src/core/place.rs app/src-tauri/src/core/mod.rs app/src-tauri/src/adapters/window.rs app/src-tauri/src/lib.rs
git commit -m "P5: kose/monitor secimi — core/place.rs testli, window.rs okuyor"
```

---

## Task 4: `DomainEvent::ConfigChanged` + `Engine::reconfigure` (TDD)

Zamanlama ve kural ayarları `Engine`'e **mpsc üzerinden** girer — başka bir thread `Engine`'e dokunmaz. `ConfigChanged` bir tetik değildir: kutuyu açmaz, hiçbir efekt üretmez.

**Files:**
- Modify: `app/src-tauri/src/core/event.rs`
- Modify: `app/src-tauri/src/core/engine.rs`
- Modify: `app/src-tauri/src/core/visibility.rs`
- Modify: `app/src-tauri/src/core/rules.rs`

**Interfaces:**
- Consumes: `EngineConfig` (Task 2'de `Clone, Copy, Debug, PartialEq, Eq` oldu)
- Produces:
  - `DomainEvent::ConfigChanged(EngineConfig)`
  - `Engine::reconfigure(&mut self, cfg: EngineConfig)`
  - `VisibilityMachine::reconfigure(&mut self, show_ms: u64, fade_ms: u64, coalesce_ms: u64)`
  - `Gatekeeper::reconfigure(&mut self, cfg: RuleConfig)`
  - `DropReason::Config`

- [ ] **Step 1: Başarısız testleri yaz**

`app/src-tauri/src/core/engine.rs`'in `#[cfg(test)] mod tests` bloğunun içine ekle (mevcut testlerin yardımcılarını kullan; `engine.rs`'teki mevcut testlerin bir olay üretme biçimi varsa **ona uy**, aşağıdaki `medya(...)` adı yer tutucudur — dosyadaki gerçek yardımcının adını kullan):

```rust
    #[test]
    fn config_degisimi_kutuyu_acmaz() {
        let mut e = Engine::new(EngineConfig::default());
        let efektler = e.handle(
            DomainEvent::ConfigChanged(EngineConfig {
                show_ms: 5_000,
                ..EngineConfig::default()
            }),
            0,
        );
        assert!(efektler.is_empty(), "ayar degisimi tetik degildir");
        assert!(!e.is_on_screen());
    }

    #[test]
    fn yeni_show_ms_sonraki_tetikte_gecerli() {
        let mut e = Engine::new(EngineConfig::default());
        e.handle(
            DomainEvent::ConfigChanged(EngineConfig {
                show_ms: 5_000,
                ..EngineConfig::default()
            }),
            0,
        );
        // Ilk olay yutulur (1. gurultu kurali), ikincisi tetikler.
        e.handle(medya("A", "playing"), 10);
        e.handle(medya("B", "playing"), 20);
        assert!(e.is_on_screen());

        // Varsayilan 2500 ms'de hala ekranda olmali: yeni sure 5000.
        e.tick(2_600);
        assert!(e.is_on_screen(), "yeni show_ms uygulanmamis");

        e.tick(5_100 + EngineConfig::default().fade_ms);
        assert!(!e.is_on_screen());
    }

    #[test]
    fn config_degisimi_gurultu_gecmisini_silmez() {
        // "Ilk olayi yut" kurali ayar degisiminde sifirlanmamali; yoksa
        // her ayar degisiminden sonra bir olay bosa gider.
        let mut e = Engine::new(EngineConfig::default());
        e.handle(medya("A", "playing"), 0);      // yutulur
        e.handle(DomainEvent::ConfigChanged(EngineConfig::default()), 5);
        let efektler = e.handle(medya("B", "playing"), 10);
        assert!(!efektler.is_empty(), "ayar degisimi ilk-olay yutmasini geri getirmis");
    }
```

- [ ] **Step 2: Testlerin başarısız olduğunu gör**

```bash
cd app/src-tauri && cargo test config_degisimi
```

Beklenen: derleme hatası — `DomainEvent::ConfigChanged` yok.

- [ ] **Step 3: `DomainEvent`'e varyantı ekle**

`app/src-tauri/src/core/event.rs`, `Volume(VolumeSnapshot),` satırının altına:

```rust
    /// Tepsi menüsü veya ayar dosyası değişti. **Tetik değildir**, hiçbir efekt
    /// üretmez — `Engine`'e yalnızca core thread'i dokunsun diye bu kanaldan
    /// geçiyor (`docs/superpowers/specs/2026-08-30-p5-paketleme-design.md` § 2).
    ConfigChanged(super::engine::EngineConfig),
```

- [ ] **Step 4: `reconfigure` fonksiyonlarını yaz**

`app/src-tauri/src/core/visibility.rs`, `impl VisibilityMachine` bloğunun içine:

```rust
    /// Yeni süreler **bir sonraki tetikten** itibaren geçerli; ekrandaki kutunun
    /// deadline'ı geriye dönük değiştirilmez (kutu ortada zıplamasın).
    pub fn reconfigure(&mut self, show_ms: u64, fade_ms: u64, coalesce_ms: u64) {
        self.show_ms = show_ms;
        self.fade_ms = fade_ms;
        self.coalesce_ms = coalesce_ms;
    }
```

**Alan adları:** `VisibilityMachine::new(show_ms, fade_ms, coalesce_ms)` bu üç değeri saklıyor; alan adları farklıysa gerçek adları kullan.

`app/src-tauri/src/core/rules.rs`, `impl Gatekeeper` bloğunun içine:

```rust
    /// Yalnızca yapılandırma değişir; **görülmüşlük geçmişi korunur** — yoksa
    /// her ayar değişiminden sonra "ilk olayı yut" kuralı bir olayı boşa yer.
    pub fn reconfigure(&mut self, cfg: RuleConfig) {
        self.cfg = cfg;
    }
```

`app/src-tauri/src/core/rules.rs`, `Gatekeeper::judge` içindeki `match`'e yeni bir kol (mevcut `SessionSwitched | SessionGone` kolunun altına):

```rust
            DomainEvent::ConfigChanged(_) => Verdict::Drop(DropReason::Config),
```

ve `DropReason` enum'una:

```rust
    /// Ayar değişimi — olay değil, tetik hiç değil.
    Config,
```

- [ ] **Step 5: `Engine::handle`'a erken dönüşü ekle**

`app/src-tauri/src/core/engine.rs`, `pub fn handle` gövdesinin **ilk satırı** olarak:

```rust
    pub fn handle(&mut self, ev: DomainEvent, now_ms: u64) -> Vec<Effect> {
        // Ayar değişimi durumu güncellemez, kural işletmez, efekt üretmez.
        if let DomainEvent::ConfigChanged(cfg) = ev {
            self.reconfigure(cfg);
            return Vec::new();
        }

        let changed = self.merge(&ev);
        // ...gerisi aynen...
```

ve `impl Engine` bloğuna:

```rust
    pub fn reconfigure(&mut self, cfg: EngineConfig) {
        self.rules.reconfigure(cfg.rules);
        self.vis.reconfigure(cfg.show_ms, cfg.fade_ms, cfg.coalesce_ms);
    }
```

`merge` ve `kind_of` fonksiyonlarında `DomainEvent` üzerinde exhaustive `match` varsa derleyici uyaracak; oralara da `DomainEvent::ConfigChanged(_) => { /* erken dönüldü, buraya gelinmez */ }` (veya `false` / `None`) kolunu ekle.

- [ ] **Step 6: Testlerin geçtiğini gör**

```bash
cd app/src-tauri && cargo test
```

Beklenen: **34 test** geçiyor (31 + 3).

- [ ] **Step 7: Commit**

```bash
git add app/src-tauri/src/core/
git commit -m "P5: DomainEvent::ConfigChanged — ayar mpsc uzerinden engine'e giriyor"
```

---

## Task 5: `adapters/config_file.rs` — yükle, kaydet, aç

**Files:**
- Create: `app/src-tauri/src/adapters/config_file.rs`
- Modify: `app/src-tauri/src/adapters/mod.rs`
- Modify: `app/src-tauri/Cargo.toml`

**Interfaces:**
- Consumes: `core::Config` (Task 2)
- Produces:
  - `adapters::config_file::path() -> Option<PathBuf>`
  - `adapters::config_file::load() -> Config`
  - `adapters::config_file::save(cfg: &Config)`
  - `adapters::config_file::reveal(cfg: &Config)` — dosyayı varsayılan editörde açar, yoksa önce `cfg` ile yazar

- [ ] **Step 1: `Cargo.toml`'a `Win32_UI_Shell` feature'ını ekle**

`app/src-tauri/Cargo.toml`, `windows = { version = "0.61", features = [...] }` listesine, alfabetik yerine (liste zaten alfabetik) `Win32_UI_Shell` satırını `Win32_UI_Shell_PropertiesSystem`'in **üstüne**:

```toml
  # ShellExecuteW (ayar dosyasini ac) ve IShellLinkW (autostart kisayolu).
  "Win32_UI_Shell",
```

- [ ] **Step 2: `config_file.rs`'i yaz**

```rust
//! Ayar dosyasının okunması ve yazılması. Karar `core/config.rs`'te, burada
//! yalnızca yol, dosya ve kabuk çağrısı var.
//!
//! **Yol `%APPDATA%\neon-flyout\config.json`.** Tauri'nin `app_config_dir()`
//! fonksiyonu bundle identifier'ı kullanıp `%APPDATA%\dev.neonflyout.app` verir;
//! kullanıcının elle düzenleyeceği bir dosya için okunaksız.

use std::path::PathBuf;

use windows::core::PCWSTR;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

use crate::core::Config;

const KLASOR: &str = "neon-flyout";
const DOSYA: &str = "config.json";

pub fn path() -> Option<PathBuf> {
    let appdata = std::env::var_os("APPDATA")?;
    Some(PathBuf::from(appdata).join(KLASOR).join(DOSYA))
}

/// Dosya yoksa **oluşturulmaz** — varsayılanlarla çalışılır. Bozuksa yine
/// varsayılan döner ve dosyaya dokunulmaz (kullanıcının yazdığı kaybolmasın).
pub fn load() -> Config {
    let Some(p) = path() else {
        eprintln!("[config] %APPDATA% okunamadi; varsayilanlar kullaniliyor");
        return Config::default();
    };
    match std::fs::read_to_string(&p) {
        Ok(s) => Config::from_json_str(&s),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Config::default(),
        Err(e) => {
            eprintln!("[config] {} okunamadi ({e}); varsayilanlar", p.display());
            Config::default()
        }
    }
}

/// Klasörü gerekiyorsa oluşturur. Yazma hatası uygulamayı durdurmaz: değişiklik
/// o oturumda zaten geçerli, yalnızca kalıcı olmaz.
pub fn save(cfg: &Config) {
    let Some(p) = path() else { return };
    if let Some(dir) = p.parent() {
        if let Err(e) = std::fs::create_dir_all(dir) {
            eprintln!("[config] {} olusturulamadi ({e})", dir.display());
            return;
        }
    }
    if let Err(e) = std::fs::write(&p, cfg.to_json_string()) {
        eprintln!("[config] {} yazilamadi ({e})", p.display());
    }
}

/// "Ayar dosyasını aç". Dosya yoksa önce mevcut ayarla yazılır — kullanıcı boş
/// bir hata değil, düzenlenebilir bir dosya görsün.
pub fn reveal(cfg: &Config) {
    let Some(p) = path() else { return };
    if !p.exists() {
        save(cfg);
    }
    let mut yol: Vec<u16> = p.as_os_str().encode_wide().collect();
    yol.push(0);
    let mut op: Vec<u16> = "open".encode_utf16().collect();
    op.push(0);
    unsafe {
        // Donus degeri HINSTANCE; <= 32 hata demek ama kullaniciya
        // gosterecegimiz bir sey yok, sessizce gec.
        let _ = ShellExecuteW(
            None,
            PCWSTR(op.as_ptr()),
            PCWSTR(yol.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
    }
}
```

`encode_wide` için dosyanın başına:

```rust
use std::os::windows::ffi::OsStrExt;
```

`adapters/mod.rs`'e (alfabetik sırada, `audio`'nun altına):

```rust
pub mod config_file;
```

- [ ] **Step 3: Derlendiğini doğrula**

```bash
cd app/src-tauri && cargo build
```

Beklenen: temiz derleme. `ShellExecuteW`'nin imzası `windows` 0.61'de farklıysa (ilk parametre `Option<HWND>` yerine `HWND` olabilir) derleyici hatası bunu söyler — imzayı hataya göre düzelt, davranışı değiştirme.

- [ ] **Step 4: Testleri koştur ve commit**

```bash
cd app/src-tauri && cargo test
git add app/src-tauri/src/adapters/config_file.rs app/src-tauri/src/adapters/mod.rs app/src-tauri/Cargo.toml
git commit -m "P5: config_file adapteri — %APPDATA%/neon-flyout/config.json"
```

Beklenen: 34 test geçiyor (yeni test yok, bu katman I/O).

---

## Task 6: `adapters/autostart.rs` — Başlangıç klasörü kısayolu

**Files:**
- Create: `app/src-tauri/src/adapters/autostart.rs`
- Modify: `app/src-tauri/src/adapters/mod.rs`

**Interfaces:**
- Consumes: yok
- Produces:
  - `adapters::autostart::enabled() -> bool`
  - `adapters::autostart::set(on: bool) -> bool` — yeni durumu döner (başarısızlıkta eski durum)

- [ ] **Step 1: `autostart.rs`'i yaz**

```rust
//! "Windows ile başlat" — Başlangıç klasörüne kısayol.
//!
//! **Gerçek kaynak dosyanın kendisidir**, `config.json` değil: kullanıcı kısayolu
//! klasörden elle silerse tepsi menüsü doğruyu gösterir.
//!
//! Dev'de kısayol **debug exe'sini** gösterir; `npm run tauri dev` ile denerken
//! bu beklenen davranış.

use std::os::windows::ffi::OsStrExt;
use std::path::PathBuf;

use windows::core::{Interface, PCWSTR};
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, IPersistFile, CLSCTX_INPROC_SERVER,
    COINIT_APARTMENTTHREADED,
};
use windows::Win32::UI::Shell::{
    IShellLinkW, SHGetKnownFolderPath, ShellLink, FOLDERID_Startup, KF_FLAG_DEFAULT,
};

const KISAYOL: &str = "neon-flyout.lnk";

fn wide(s: &std::ffi::OsStr) -> Vec<u16> {
    let mut v: Vec<u16> = s.encode_wide().collect();
    v.push(0);
    v
}

/// Başlangıç klasörü. `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup`
/// — sabit yol yerine `SHGetKnownFolderPath`, yerelleştirilmiş kurulumlarda da
/// doğru olsun diye.
fn startup_dir() -> Option<PathBuf> {
    unsafe {
        let p = SHGetKnownFolderPath(&FOLDERID_Startup, KF_FLAG_DEFAULT, None).ok()?;
        let s = p.to_string().ok()?;
        CoTaskMemFree(Some(p.0 as *const _));
        Some(PathBuf::from(s))
    }
}

fn kisayol_yolu() -> Option<PathBuf> {
    Some(startup_dir()?.join(KISAYOL))
}

pub fn enabled() -> bool {
    kisayol_yolu().is_some_and(|p| p.exists())
}

/// `true` istenirse kısayolu yazar, `false` istenirse siler. Dönüş **gerçekleşen**
/// durumdur — başarısızlıkta menü yanlış onay kutusu göstermesin.
pub fn set(on: bool) -> bool {
    let Some(p) = kisayol_yolu() else {
        return enabled();
    };
    if on {
        if let Err(e) = yaz(&p) {
            eprintln!("[autostart] kisayol yazilamadi ({e})");
        }
    } else if let Err(e) = std::fs::remove_file(&p) {
        if e.kind() != std::io::ErrorKind::NotFound {
            eprintln!("[autostart] kisayol silinemedi ({e})");
        }
    }
    enabled()
}

fn yaz(hedef: &std::path::Path) -> windows::core::Result<()> {
    let exe = std::env::current_exe().map_err(|e| {
        windows::core::Error::new(windows::Win32::Foundation::E_FAIL, e.to_string())
    })?;
    let dir = exe.parent().unwrap_or(&exe).to_path_buf();

    // Omur tuzagi: PCWSTR(wide(..).as_ptr()) yazilirsa gecici Vec satir sonunda
    // duser ve isaretci sarkar — derleyici bunu YAKALAMAZ. Her tampon once bir
    // degiskende tutuluyor.
    let exe_w = wide(exe.as_os_str());
    let dir_w = wide(dir.as_os_str());
    let desc_w = wide(std::ffi::OsStr::new("neon-flyout — medya flyout'u"));
    let hedef_w = wide(hedef.as_os_str());

    unsafe {
        // Tauri'nin ana thread'i zaten bir apartment'ta; RPC_E_CHANGED_MODE
        // beklenen ve zararsiz, yut.
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER)?;
        link.SetPath(PCWSTR(exe_w.as_ptr()))?;
        link.SetWorkingDirectory(PCWSTR(dir_w.as_ptr()))?;
        link.SetDescription(PCWSTR(desc_w.as_ptr()))?;

        let file: IPersistFile = link.cast()?;
        file.Save(PCWSTR(hedef_w.as_ptr()), true)?;
    }
    Ok(())
}
```

`adapters/mod.rs`'e (alfabetik, `audio`'nun üstüne):

```rust
pub mod autostart;
```

- [ ] **Step 2: Derle**

```bash
cd app/src-tauri && cargo build
```

Beklenen: temiz derleme. `SHGetKnownFolderPath`'in `KF_FLAG_DEFAULT` sabiti `windows` 0.61'de `KNOWN_FOLDER_FLAG(0)` olabilir; derleyici hatası yönlendirir.

- [ ] **Step 3: Elle doğrula**

Geçici bir test için `lib.rs`'in `setup` bloğuna **geçici** iki satır ekle, `npm run tauri dev` çalıştır, sonra satırları **sil**:

```rust
            eprintln!("[autostart] once: {}", adapters::autostart::enabled());
            eprintln!("[autostart] set(true) -> {}", adapters::autostart::set(true));
```

Doğrula: `%APPDATA%\Microsoft\Windows\Start Menu\Programs\Startup` klasöründe `neon-flyout.lnk` oluştu, özelliklerinde hedef debug exe'yi gösteriyor. Sonra elle sil.

- [ ] **Step 4: Commit**

```bash
git add app/src-tauri/src/adapters/autostart.rs app/src-tauri/src/adapters/mod.rs
git commit -m "P5: autostart adapteri — Baslangic klasorune kisayol"
```

---

## Task 7: Frontend — `ayarlar` komutu ve `config` kanalı

WebView, backend'in emit'inden sonra yüklenirse temayı kaçırır (P3'te `Frame` için aynısı yaşandı). Çözüm: frontend init'te ayarı **kendisi çeker**; emit yalnızca sonraki değişiklikler için.

**Files:**
- Modify: `app/src-tauri/src/lib.rs`
- Modify: `app/src/main.ts`

**Interfaces:**
- Consumes: `core::Config`, `adapters::config_file::load` (Task 2, 5)
- Produces:
  - Tauri state: `Mutex<Config>` (Task 8'in tepsi kodu bunu değiştirecek)
  - Tauri command: `ayarlar() -> UiConfig` — `{ theme: "cyber" | "neon" }`
  - Emit kanalı: `"config"`, payload `UiConfig`

- [ ] **Step 1: Backend tarafını yaz**

`app/src-tauri/src/lib.rs`, `KANAL_TEPE` sabitinin altına:

```rust
/// Ayarın WebView'i ilgilendiren kısmı. Köşe, süre ve kurallar frontend'e hiç
/// gitmiyor — onlar backend'in işi.
const KANAL_AYAR: &str = "config";

#[derive(Clone, Serialize)]
pub struct UiConfig {
    pub theme: crate::core::Theme,
}

/// Ayarın yetkili kopyası. Tepsi menüsü ve dosya yükleme burayı değiştirir,
/// sonra üç tüketiciye **iter** — kimse buradan sürekli okumaz.
pub struct SharedConfig(pub std::sync::Mutex<crate::core::Config>);

#[tauri::command]
fn ayarlar(state: tauri::State<'_, SharedConfig>) -> UiConfig {
    let cfg = state.0.lock().expect("config mutex zehirli");
    UiConfig { theme: cfg.theme }
}
```

`run()` içinde `tauri::Builder::default()` zincirine, `.setup(...)`'ın **üstüne**:

```rust
        .manage(SharedConfig(std::sync::Mutex::new(adapters::config_file::load())))
        .invoke_handler(tauri::generate_handler![ayarlar])
```

`setup` bloğunun içinde, `adapters::window::apply_styles(hwnd);` satırının **hemen üstüne** — pencere ilk kez konumlanmadan önce köşe tercihi bilinsin:

```rust
            let cfg = *app.state::<SharedConfig>().0.lock().expect("config mutex zehirli");
            adapters::window::set_placement(cfg.corner, cfg.monitor);
```

ve core thread'i başlatan bloktan **önce**, engine'e ilk ayarı ver — `Engine::new(EngineConfig::default())` yerine ayarla kurulsun. En temiz yol `core_loop`'a config parametresi geçirmek:

```rust
            let ilk = cfg.engine();
            std::thread::Builder::new()
                .name("core".into())
                .spawn(move || core_loop(handle, hwnd, meter, rx, ilk))?;
```

ve `core_loop` imzası:

```rust
fn core_loop(
    app: AppHandle,
    hwnd: isize,
    meter: SyncSender<MeterCmd>,
    rx: Receiver<DomainEvent>,
    cfg: EngineConfig,
) {
    let start = Instant::now();
    let now = move || start.elapsed().as_millis() as u64;
    let mut engine = Engine::new(cfg);
    // ...gerisi aynen...
```

`use` satırına `Theme`/`Config` ekle:

```rust
use crate::core::{Config, DomainEvent, Effect, Engine, EngineConfig, Theme};
```

(Kullanılmayanları derleyici uyarısına göre ayıkla.)

- [ ] **Step 2: Frontend tarafını yaz**

`app/src/main.ts`, import bloğuna:

```ts
import { invoke } from "@tauri-apps/api/core";
```

`VARSAYILAN_TEMA` yorumunu güncelle:

```ts
/** Backend `ayarlar` komutu gelene kadarki ilk boyama. Canlı deneme: `__tema()`. */
const VARSAYILAN_TEMA: Theme = "cyber";
```

`interface Peak` bloğunun altına:

```ts
/** Ayarın WebView'i ilgilendiren kısmı — `lib.rs` → `UiConfig`. */
interface UiConfig {
  theme: Theme;
}
```

Dosyanın sonundaki "bağlan" bölümünü şu hâle getir:

```ts
// Temayı önce senkron uygula: WebView backend'den önce boyayabilir, kutu
// temasız çıkmasın. Ayar gelince üstüne yazılır.
temaUygula(VARSAYILAN_TEMA);
// Emit'i beklemiyoruz — WebView emit'ten sonra yüklenirse temayı kaçırırdı
// (P3'te `Frame` için aynısı yaşandı). Init'te ayarı kendimiz çekiyoruz.
invoke<UiConfig>("ayarlar")
  .then((c) => temaUygula(c.theme))
  .catch(() => {});

listen<UiConfig>("config", (e) => temaUygula(e.payload.theme));
listen<Frame>("flyout", (e) => render(e.payload));
listen<Peak>("peak", (e) => tepeYaz(e.payload.peak));
```

- [ ] **Step 3: Çalıştır ve doğrula**

```bash
cd app && npm run tauri dev
```

`config.json`'ı elle `%APPDATA%\neon-flyout\config.json` olarak `{"theme":"neon"}` içeriğiyle oluştur, uygulamayı yeniden başlat, bir parça değiştir → **kutu neon temasında çıkmalı**. Sonra `{"corner":"bottom-left"}` ekle, yeniden başlat → kutu sol altta çıkmalı.

- [ ] **Step 4: Commit**

```bash
git add app/src-tauri/src/lib.rs app/src/main.ts
git commit -m "P5: ayar backend'de yukleniyor, frontend init'te cekiyor"
```

---

## Task 8: `adapters/tray.rs` — tepsi ikonu ve menü

Her şeyi birleştiren task. Menüden gelen her değişiklik: (1) `SharedConfig`'i günceller, (2) ilgili tüketiciye iter, (3) dosyaya yazar — bu sırayla, çünkü yazma başarısız olsa bile değişiklik o oturumda geçerli olmalı.

**Files:**
- Create: `app/src-tauri/src/adapters/tray.rs`
- Modify: `app/src-tauri/src/adapters/mod.rs`
- Modify: `app/src-tauri/src/lib.rs`
- Modify: `app/src-tauri/Cargo.toml`

**Interfaces:**
- Consumes: `SharedConfig`, `UiConfig`, `KANAL_AYAR`, `config_file`, `autostart`, `window::set_placement`, `DomainEvent::ConfigChanged` (Task 2–7)
- Produces: `adapters::tray::init(app: &AppHandle, tx: Sender<DomainEvent>) -> tauri::Result<()>`

- [ ] **Step 1: `tray-icon` feature'ını aç**

`app/src-tauri/Cargo.toml`:

```toml
tauri = { version = "2", features = ["tray-icon"] }
```

- [ ] **Step 2: `tray.rs`'i yaz**

```rust
//! Tepsi ikonu ve menüsü. Uygulamanın **tek** kullanıcı arayüzü giriş noktası —
//! kutunun kendisi tıklanabilir değil (`WS_EX_TRANSPARENT`).
//!
//! Menü değişiminde sıra: (1) yetkili Config güncellenir, (2) ilgili tüketiciye
//! itilir, (3) dosyaya yazılır. Dosyaya yazma başarısız olsa bile değişiklik o
//! oturumda geçerli olur.
//!
//! **Radyo düğmesi yok:** Tauri v2 menüsünde radio öğesi bulunmuyor; tema ve
//! köşe onay işaretli öğelerle gösteriliyor ve karşılıklı dışlama elle yapılıyor.

use std::sync::mpsc::Sender;

use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::TrayIconBuilder;
use tauri::{AppHandle, Emitter, Manager};

use crate::core::{Corner, DomainEvent, MonitorPick, Theme};
use crate::{SharedConfig, UiConfig, KANAL_AYAR};

use super::{autostart, config_file, window};

const TEMALAR: [(&str, Theme, &str); 2] = [
    ("theme:cyber", Theme::Cyber, "cyber"),
    ("theme:neon", Theme::Neon, "neon"),
];

const KOSELER: [(&str, Corner, &str); 4] = [
    ("corner:top-right", Corner::TopRight, "sağ üst"),
    ("corner:top-left", Corner::TopLeft, "sol üst"),
    ("corner:bottom-right", Corner::BottomRight, "sağ alt"),
    ("corner:bottom-left", Corner::BottomLeft, "sol alt"),
];

pub fn init(app: &AppHandle, tx: Sender<DomainEvent>) -> tauri::Result<()> {
    let cfg = *app.state::<SharedConfig>().0.lock().expect("config mutex zehirli");

    let tema_ogeleri: Vec<CheckMenuItem<_>> = TEMALAR
        .iter()
        .map(|(id, t, ad)| {
            CheckMenuItem::with_id(app, id, ad, true, cfg.theme == *t, None::<&str>)
        })
        .collect::<tauri::Result<_>>()?;
    let kose_ogeleri: Vec<CheckMenuItem<_>> = KOSELER
        .iter()
        .map(|(id, c, ad)| {
            CheckMenuItem::with_id(app, id, ad, true, cfg.corner == *c, None::<&str>)
        })
        .collect::<tauri::Result<_>>()?;

    let tema_ref: Vec<&dyn tauri::menu::IsMenuItem<_>> =
        tema_ogeleri.iter().map(|i| i as &dyn tauri::menu::IsMenuItem<_>).collect();
    let kose_ref: Vec<&dyn tauri::menu::IsMenuItem<_>> =
        kose_ogeleri.iter().map(|i| i as &dyn tauri::menu::IsMenuItem<_>).collect();

    let tema_alt = Submenu::with_items(app, "Tema", true, &tema_ref)?;
    let kose_alt = Submenu::with_items(app, "Köşe", true, &kose_ref)?;

    let baslat = CheckMenuItem::with_id(
        app,
        "autostart",
        "Windows ile başlat",
        true,
        autostart::enabled(),
        None::<&str>,
    )?;
    let ac = MenuItem::with_id(app, "open", "Ayar dosyasını aç", true, None::<&str>)?;
    let yenile = MenuItem::with_id(app, "reload", "Ayarları yeniden yükle", true, None::<&str>)?;
    let cik = MenuItem::with_id(app, "quit", "Çıkış", true, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &tema_alt,
            &kose_alt,
            &PredefinedMenuItem::separator(app)?,
            &baslat,
            &ac,
            &yenile,
            &PredefinedMenuItem::separator(app)?,
            &cik,
        ],
    )?;

    // Menü öğelerini olay işleyicisine taşı: onay işaretlerini elle yönetiyoruz.
    let temalar = tema_ogeleri;
    let koseler = kose_ogeleri;
    let baslat_ogesi = baslat;

    TrayIconBuilder::with_id("main")
        .icon(app.default_window_icon().expect("varsayilan ikon yok").clone())
        .tooltip("neon-flyout")
        .menu(&menu)
        // Sol tıklamada gösterecek bir pencere yok: kutu olaya tepki verir.
        .show_menu_on_left_click(false)
        .on_menu_event(move |app, ev| {
            let id = ev.id().as_ref();
            match id {
                "quit" => {
                    // Tek düzgün çıkış yolu: RunEvent::Exit → native_flyout::restore()
                    app.exit(0);
                }
                "open" => {
                    let cfg = *app.state::<SharedConfig>().0.lock().expect("zehirli");
                    config_file::reveal(&cfg);
                }
                "reload" => {
                    let yeni = config_file::load();
                    uygula(app, &tx, yeni);
                    isaretle(&temalar, TEMALAR.iter().map(|t| t.0), &format!("theme:{}", ad_of_theme(yeni.theme)));
                    isaretle(&koseler, KOSELER.iter().map(|k| k.0), &format!("corner:{}", ad_of_corner(yeni.corner)));
                    let _ = baslat_ogesi.set_checked(autostart::enabled());
                }
                "autostart" => {
                    let istenen = baslat_ogesi.is_checked().unwrap_or(false);
                    let gercek = autostart::set(istenen);
                    // Dosya yazılamadıysa onay kutusu yalan söylemesin.
                    let _ = baslat_ogesi.set_checked(gercek);
                    let mut cfg = *app.state::<SharedConfig>().0.lock().expect("zehirli");
                    cfg.autostart = gercek;
                    kaydet(app, cfg);
                }
                _ => {
                    if let Some((_, t, _)) = TEMALAR.iter().find(|(i, _, _)| *i == id) {
                        let mut cfg = *app.state::<SharedConfig>().0.lock().expect("zehirli");
                        cfg.theme = *t;
                        uygula(app, &tx, cfg);
                        isaretle(&temalar, TEMALAR.iter().map(|x| x.0), id);
                    } else if let Some((_, c, _)) = KOSELER.iter().find(|(i, _, _)| *i == id) {
                        let mut cfg = *app.state::<SharedConfig>().0.lock().expect("zehirli");
                        cfg.corner = *c;
                        uygula(app, &tx, cfg);
                        isaretle(&koseler, KOSELER.iter().map(|x| x.0), id);
                    }
                }
            }
        })
        .build(app)?;

    Ok(())
}

/// Onay işaretlerini karşılıklı dışlamalı hâle getirir: yalnızca `secili` işaretli.
fn isaretle<R: tauri::Runtime>(
    ogeler: &[CheckMenuItem<R>],
    idler: impl Iterator<Item = &'static str>,
    secili: &str,
) {
    for (oge, id) in ogeler.iter().zip(idler) {
        let _ = oge.set_checked(id == secili);
    }
}

fn ad_of_theme(t: Theme) -> &'static str {
    match t {
        Theme::Cyber => "cyber",
        Theme::Neon => "neon",
    }
}

fn ad_of_corner(c: Corner) -> &'static str {
    match c {
        Corner::TopRight => "top-right",
        Corner::TopLeft => "top-left",
        Corner::BottomRight => "bottom-right",
        Corner::BottomLeft => "bottom-left",
    }
}

/// Yetkili Config'i günceller, üç tüketiciye iter, dosyaya yazar — bu sırayla.
fn uygula(app: &AppHandle, tx: &Sender<DomainEvent>, cfg: crate::core::Config) {
    {
        let mut g = app.state::<SharedConfig>().0.lock().expect("zehirli");
        *g = cfg;
    }
    // 1. Tema → WebView
    let _ = app.emit(KANAL_AYAR, UiConfig { theme: cfg.theme });
    // 2. Köşe/monitör → pencere adapteri (etkisi bir sonraki gösterimde)
    window::set_placement(cfg.corner, cfg.monitor);
    // 3. Zamanlama/kural → core thread'i, mpsc üzerinden
    let _ = tx.send(DomainEvent::ConfigChanged(cfg.engine()));
    config_file::save(&cfg);
}

fn kaydet(app: &AppHandle, cfg: crate::core::Config) {
    {
        let mut g = app.state::<SharedConfig>().0.lock().expect("zehirli");
        *g = cfg;
    }
    config_file::save(&cfg);
}
```

`adapters/mod.rs`'e:

```rust
pub mod tray;
```

- [ ] **Step 3: `lib.rs`'ten çağır**

`setup` bloğunda, `adapters::smtc::spawn(tx.clone())?;` satırının **üstüne**:

```rust
            adapters::tray::init(app.handle(), tx.clone())?;
```

- [ ] **Step 4: Derle ve tip hatalarını düzelt**

```bash
cd app/src-tauri && cargo build
```

Tauri 2.11'in menü API'si (özellikle `Submenu::with_items` ve `IsMenuItem` dönüşümleri) yukarıdaki koddan farklı imza isteyebilir. **Davranışı değiştirme, imzayı düzelt.** Takılırsan güncel API için `context7:docs` skill'iyle `tauri` menü/tray dokümanını çek.

- [ ] **Step 5: Elle doğrula**

```bash
cd app && npm run tauri dev
```

Sırayla dene ve hepsinin çalıştığını gör:
- Tepsi ikonu görünüyor, sağ tıkla menü açılıyor.
- Tema → neon: bir parça değiştir, kutu neon çıkıyor; menüde işaret neon'da.
- Köşe → sol alt: bir sonraki kutu sol altta.
- "Ayar dosyasını aç": Not Defteri (veya varsayılan editör) `config.json`'ı açıyor ve içeriği menüden yaptığın değişiklikleri gösteriyor.
- Dosyada `"show_ms": 6000` yap, kaydet, "Ayarları yeniden yükle" → kutu 6 saniye kalıyor.
- "Windows ile başlat" işaretle → Başlangıç klasöründe kısayol var; kaldır → yok.
- "Çıkış" → uygulama kapanıyor **ve Windows'un kendi medya flyout'u geri geliyor** (ses tuşuna bas).

- [ ] **Step 6: Commit**

```bash
cd /c/dev/v1-kasa/01-Projects/neon-flyout
git add app/src-tauri/src/adapters/tray.rs app/src-tauri/src/adapters/mod.rs app/src-tauri/src/lib.rs app/src-tauri/Cargo.toml app/src-tauri/Cargo.lock
git commit -m "P5: tepsi ikonu ve menu — tema/kose/autostart/cikis"
```

---

## Task 9: `native_flyout` ex-style maskesi — kill-9 sonrası bozulan `restore()`

`suppress()` o anki ex-style'ı kaydediyor. Süreç öldürülüp uygulama yeniden başlarsa kabuğun penceresi **hâlâ bastırılmış** durumdadır ve kaydedilen stil bastırma bitlerini içerir; sonraki düzgün çıkışta `restore()` "bastırılmış stili" geri yükler.

**Files:**
- Modify: `app/src-tauri/src/adapters/native_flyout.rs:89`
- Modify: `docs/P4-ARASTIRMA.md`

**Interfaces:**
- Consumes: yok
- Produces: davranış düzeltmesi; yeni API yok

- [ ] **Step 1: Varsayımı ölç — bu bitler özgün stilde var mı?**

Düzeltme, kabuğun penceresinin **bastırılmamış** hâlinde `WS_EX_LAYERED` ve `WS_EX_TRANSPARENT` bulunmadığı varsayımına dayanıyor. Tahmin etme, ölç.

Uygulama **çalışmıyorken** (ve daha önce bastırılmamışken — gerekirse `spikes/p4-probe/p4-restore.ps1` ile temizle) `spikes/p4-probe/` altındaki sondayı çalıştır ve host penceresinin ex-style'ını oku. Sonda yoksa şu PowerShell'i kullan:

```powershell
Add-Type @'
using System;using System.Runtime.InteropServices;
public class W{
 [DllImport("user32.dll",CharSet=CharSet.Unicode)] public static extern IntPtr FindWindowExW(IntPtr p,IntPtr c,string cls,string w);
 [DllImport("user32.dll")] public static extern IntPtr GetWindowLongPtrW(IntPtr h,int i);
}
'@
$h=[W]::FindWindowExW([IntPtr]::Zero,[IntPtr]::Zero,"XamlExplorerHostIslandWindow",$null)
"hwnd=$h exstyle=0x{0:X}" -f [int64][W]::GetWindowLongPtrW($h,-20)
```

`WS_EX_LAYERED = 0x00080000`, `WS_EX_TRANSPARENT = 0x00000020`.

Beklenen: ikisi de **yok**. Varsa **dur** — maskeleme yanlış olur; spec § 6'yı "ilk görülen stili kalıcı yaz" yoluna revize et ve kullanıcıya haber ver.

**Not:** `FindWindowExW` kullan, `EnumWindows` **değil** — `EnumWindows` band 18'deki pencereleri listelemiyor ve P4'ün ilk ölçüm turu bu yüzden yanlış sonuca vardı (`docs/P4-ARASTIRMA.md` § 3.5).

- [ ] **Step 2: Maskeyi uygula**

`app/src-tauri/src/adapters/native_flyout.rs`, `pub fn suppress()` içindeki satırı değiştir:

```rust
        // Bastirma bitlerini kaydetme: surec oldurulup uygulama yeniden
        // baslarsa pencere HALA bastirilmis olur ve bu okuma "bastirilmis
        // stili" ozgun sanip restore()'da geri yazardi.
        let ex_style = GetWindowLongPtrW(hwnd(host), GWL_EXSTYLE)
            & !(WS_EX_LAYERED.0 as isize | WS_EX_TRANSPARENT.0 as isize);
```

- [ ] **Step 3: Testler ve derleme**

```bash
cd app/src-tauri && cargo test
```

Beklenen: **34 test** geçiyor, derleme temiz.

- [ ] **Step 4: Elle doğrula — kill-9 döngüsü**

1. `npm run tauri dev` ile başlat, ses tuşuna bas → **yalnızca** bizim kutumuz çıkıyor.
2. Görev Yöneticisi'nden `neon-flyout.exe`'yi **sonlandır**.
3. Ses tuşuna bas → Windows'un kutusu **çıkmıyor** (beklenen: bastırma kalıcı).
4. Tekrar başlat, tepsi menüsünden **Çıkış**.
5. Ses tuşuna bas → **Windows'un kendi kutusu geri gelmiş olmalı.** Maskeleme olmadan bu adım başarısız olur.

- [ ] **Step 5: `docs/P4-ARASTIRMA.md`'ye notu düş**

Belgeye, bastırma bölümünün sonuna:

```markdown
**P5'te düzeltildi — kaydedilen ex-style maskeleniyor.** `suppress()` o anki
stili kaydediyordu; süreç öldürülüp uygulama yeniden başladığında pencere hâlâ
bastırılmış olduğu için kaydedilen stil `WS_EX_LAYERED|WS_EX_TRANSPARENT`
içeriyordu ve sonraki düzgün çıkışta `restore()` bastırılmış stili geri yazıyordu.
Artık bu iki bit kaydetmeden önce maskeleniyor. Özgün stilde bu bitlerin
bulunmadığı ölçülerek doğrulandı.
```

- [ ] **Step 6: Commit**

```bash
git add app/src-tauri/src/adapters/native_flyout.rs docs/P4-ARASTIRMA.md
git commit -m "P5: suppress() kaydedilen ex-style'da bastirma bitlerini maskeliyor"
```

---

## Task 10: Bundle, installer ve çıkış kriteri

**Files:**
- Modify: `app/src-tauri/tauri.conf.json`
- Modify: `docs/ROADMAP.md`
- Modify: `CLAUDE.md`
- Create: `docs/P5-SONUC.md`

**Interfaces:**
- Consumes: Task 1–9'un tamamı
- Produces: `app/src-tauri/target/release/bundle/nsis/neon-flyout_0.1.0_x64-setup.exe`

- [ ] **Step 1: `tauri.conf.json`'daki bundle bloğunu yaz**

```json
  "bundle": {
    "active": true,
    "targets": ["nsis"],
    "icon": [
      "icons/32x32.png",
      "icons/128x128.png",
      "icons/128x128@2x.png",
      "icons/icon.icns",
      "icons/icon.ico"
    ],
    "windows": {
      "webviewInstallMode": { "type": "embedBootstrapper" },
      "nsis": { "installMode": "currentUser" }
    }
  }
```

- [ ] **Step 2: Release build al**

```bash
cd app && npm run tauri build
```

Beklenen: `src-tauri/target/release/bundle/nsis/` altında `*-setup.exe`. İlk build uzun sürer (release derlemesi). NSIS araçları yoksa Tauri bunları indirir; internet gerekir.

- [ ] **Step 3: Kur ve çıkış kriterini sırayla doğrula**

Installer'ı çalıştır (SmartScreen "bilinmeyen yayıncı" uyarısı **beklenen**: imzalama sertifikası yok → "Yine de çalıştır"). Sonra **kurulu exe** üzerinde, `npm run tauri dev` ile değil:

- [ ] 1. Kurulum tamamlanıyor, uygulama Başlat menüsünden açılıyor.
- [ ] 2. Tepsi ikonu görünüyor; menü açılıyor.
- [ ] 3. Tema menüden değiştirilince bir sonraki kutu yeni temada çıkıyor.
- [ ] 4. Köşe menüden değiştirilince bir sonraki kutu yeni köşede çıkıyor.
- [ ] 5. `%APPDATA%\neon-flyout\config.json` yazılıyor; elle düzenleyip "Ayarları yeniden yükle" deyince değişiklik geçerli oluyor.
- [ ] 6. Dosyaya `{ bozuk` yazıp yeniden başlat → uygulama çökmüyor, varsayılanlarla çalışıyor, **dosyanın üstüne yazmıyor** (dosyayı tekrar aç, hâlâ bozuk içerik duruyor).
- [ ] 7. "Windows ile başlat" işaretlenince Başlangıç klasöründe kısayol oluşuyor (**hedefi kurulu exe**, debug değil), kaldırılınca siliniyor.
- [ ] 8. "Çıkış" sonrası Windows'un kendi medya flyout'u geri geliyor.
- [ ] 9. Görev Yöneticisi'nden öldür → yeniden başlat → düzgün çık → kabuğun kutusu yine geri geliyor.
- [ ] 10. **Fontlar doğru** — kutu Chakra Petch/JetBrains Mono ile çıkıyor, sistem fontuna düşmüyor. (Task 1'in kapısının gerçek sınavı bu.)
- [ ] 11. Denetim Masası'ndan kaldır → tamamlanıyor; `config.json` **kalıyor**.
- [ ] 12. `cd app/src-tauri && cargo test` → **34 test** geçiyor (taban 16 + config 9 + place 6 + ConfigChanged 3).

Bir madde başarısızsa **durma noktası orası**: sebebi bul, düzelt, o maddeden devam et.

- [ ] **Step 4: `docs/P5-SONUC.md`'yi yaz**

Diğer `P*-SONUC.md` dosyalarının biçimine uy. İçermesi gerekenler:
- Task 1'in kapı sonucu: `npm run build` fontları taşıdı mı, alias gerekti mi.
- Task 9'un ölçümü: özgün ex-style değeri (`0x...`) ve bitlerin yokluğu.
- Çıkış kriterinin 11 maddesinin sonucu.
- Installer boyutu ve SmartScreen davranışı.
- Açık kalanlar: projeye özel ikon yok, kod imzalama yok, temiz VM'de denenmedi (denendiyse sonucu).

- [ ] **Step 5: `docs/ROADMAP.md` ve `CLAUDE.md`'yi güncelle**

`ROADMAP.md` → P5 başlığını diğer aşamalar gibi `✅ TAMAMLANDI (tarih)` biçimine çevir, özgün planı `<details>` içine al, sonucu `docs/P5-SONUC.md`'ye yönlendir.

`CLAUDE.md` → "İlk oturumda sırayla yapılacaklar" listesine 8. madde olarak P5'i ekle; "Doğrulanmamış varsayımlar" bölümüne Task 9'un ölçümünü "P5'te cevaplananlar" olarak geçir; "Çalıştırma" bölümüne installer build komutunu ekle.

- [ ] **Step 6: Commit**

```bash
git add app/src-tauri/tauri.conf.json docs/P5-SONUC.md docs/ROADMAP.md CLAUDE.md
git commit -m "P5 tamamlandi: NSIS installer, tepsi, ayar dosyasi, autostart"
```

- [ ] **Step 7: Ana repodaki submodule işaretçisini güncelle**

```bash
cd /c/dev/v1-kasa
git add 01-Projects/neon-flyout
git commit -m "neon-flyout submodule: P5 tamamlandi (paketleme)"
```

Push **yapma** — `CLAUDE.md`: push için her zaman kullanıcı onayı istenir.
