# P5 — Paketleme: tasarım

**Tarih:** 2026-08-30 · **Durum:** onaylandı, uygulanmayı bekliyor
**Önkoşul:** P0–P4 tamamlandı (`docs/ROADMAP.md`)

P5 uygulamayı "geliştirici makinesinde `npm run tauri dev` ile çalışan bir şey"den
"kurulup unutulan bir tepsi uygulaması"na çeviriyor. Dört parça: **ayar dosyası**,
**tepsi ikonu**, **Windows ile başlat**, **NSIS installer**. Beşinci bir madde
P4'ten devrediyor: düzgün çıkış olmadan `restore()` çalışmıyor.

---

## 1. Kararlar (bu tasarımı sabitleyen üç seçim)

| Konu | Karar | Elenen alternatif |
|---|---|---|
| Ayar yüzeyi | Tepsi menüsü + elle düzenlenebilir `config.json` | Ayrı ayar penceresi (ikinci WebView + tasarım + IPC; `design/` altında karşılığı yok) |
| Windows ile başlat | Başlangıç klasörüne `.lnk` | HKCU `Run` anahtarı; Görev Zamanlayıcı (installer'a kaldırma-temizliği yükü) |
| Installer | NSIS + `embedBootstrapper` | Offline installer (~130 MB); MSI/WiX (per-machine + UAC, tek kullanıcılık uygulamaya gereksiz) |

---

## 2. Mimari: ayar sisteme nasıl ulaşır

**Seçilen yaklaşım: ayar, tüketicisine göre bölünür.** Tek bir paylaşılan config
nesnesi yok; her ayar yalnızca onu kullanan katmana iner.

```
                       ┌─ theme ───────────→ app.emit("config") ──→ WebView (data-theme)
config.json ─┐         │
             ├─ Config ┼─ corner, monitor ──→ window::set_placement()
tepsi menüsü ┘         │
                       └─ show_ms, fade_ms,  ─→ mpsc ──→ DomainEvent::ConfigChanged
                          coalesce_ms,               (core döngüsü uygular)
                          skip_when_foreground
```

**Neden böyle.** Mevcut mimari zaten bu çizgiden bölünmüş (`docs/ARCHITECTURE.md`):
karar core'da, çizim WebView'de, konum window adapter'da. Ayarı ayrı bir küresel
nesneye koymak dördüncü bir otorite yaratırdı.

Kritik kısıt: **`Engine`'e yalnızca core thread'i dokunur.** Zamanlama ve kural
ayarları bu yüzden `Arc<RwLock<..>>` ile değil, mevcut mpsc kanalından yeni bir
`DomainEvent::ConfigChanged(EngineConfig)` olarak geçiyor — P2'nin tek-yazıcı
disiplini (`docs/P2-SONUC.md`) korunuyor.

Tema ve konum core'un kararı değil: tema doğrudan WebView'e emit edilir, konum
`adapters/window.rs` içinde tutulan bir statikle `show()` çağrısında okunur
(`show()` zaten her gösterimde konumlandırma yapıyor, dolayısıyla köşe değişimi
ek koda gerek kalmadan bir sonraki kutuda geçerli olur).

### Elenen alternatifler

- **`Arc<RwLock<Config>>`, herkes okur.** Tek nesne olması cazip ama core'un
  tek-yazıcı disiplinini deler ve engine'i her turda config okumaya zorlar.
- **Değişince yeniden başlat.** En az kod, en kötü davranış: tepsi ikonu titrer,
  `suppress()` yeniden koşar, kısa süre kabuğun kendi kutusu geri gelir.

### Yeni dosyalar

```
app/src-tauri/src/core/config.rs           saf veri: serde + varsayılan + kırpma (BİRİM TESTLİ)
app/src-tauri/src/adapters/config_file.rs  yol çözümü, oku/yaz, "dosyayı aç"
app/src-tauri/src/adapters/tray.rs         TrayIconBuilder + menü + olay yönlendirme
app/src-tauri/src/adapters/autostart.rs    COM: IShellLinkW ile .lnk yaz/sil/var mı
```

Mimari kuralı bozulmuyor: `unsafe`/COM yalnızca `adapters/` altında
(`autostart.rs`). `core/config.rs` Windows'a dair hiçbir tip bilmez.

---

## 3. `config.json`

**Konum:** `%APPDATA%\neon-flyout\config.json`

Yol `%APPDATA%` ortam değişkeninden **doğrudan** kuruluyor. Tauri'nin
`app.path().app_config_dir()` fonksiyonu bundle identifier'ı kullanır ve
`%APPDATA%\dev.neonflyout.app` verir — kullanıcının elle düzenleyeceği bir dosya
için okunaksız bir yol, o yüzden kullanılmıyor.

```json
{
  "theme": "cyber",
  "corner": "top-right",
  "monitor": "primary",
  "show_ms": 2500,
  "fade_ms": 150,
  "coalesce_ms": 1000,
  "skip_when_foreground": false,
  "autostart": false
}
```

| Alan | Değerler | Nereye gider |
|---|---|---|
| `theme` | `cyber` \| `neon` | WebView (`data-theme`) |
| `corner` | `top-right` \| `top-left` \| `bottom-right` \| `bottom-left` | `window::set_placement` |
| `monitor` | `primary` \| `cursor` | `window::set_placement` |
| `show_ms` | 500–15000 | `EngineConfig` |
| `fade_ms` | 0–2000 | `EngineConfig` |
| `coalesce_ms` | 0–5000 | `EngineConfig` |
| `skip_when_foreground` | bool | `RuleConfig` (P1/P2'nin 4. gürültü kuralı) |
| `autostart` | bool | `adapters::autostart` — **dosya değil, kısayolun varlığı gerçek kaynaktır** (§ 5) |

Varsayılanlar `core/engine.rs`'teki mevcut sabitlerden (`SHOW_MS`, `FADE_MS`,
`COALESCE_MS`) ve `RuleConfig::default()`'tan gelir — burada yeniden yazılmaz,
oradan okunur. Uygulama sırasında sabitlerin gerçek değerleri kontrol edilecek;
yukarıdaki JSON örneğindeki sayılar açıklayıcıdır, kaynak değildir.

### Dayanıklılık kuralları

- **Eksik alan → varsayılan** (`#[serde(default)]`).
- **Tanınmayan alan → sessizce yok sayılır** (ileri uyumluluk).
- **Bozuk JSON → tamamı varsayılan + `stderr`'e tek satır uyarı, çökme yok.**
  Bozuk dosyanın **üstüne yazılmaz** — kullanıcının yazdığı içerik kaybolmasın.
- **Aralık dışı sayı → kırpılır** (tabloda verilen aralıklara). Kırpma
  `core/config.rs`'te, birim testli.
- **Dosya yoksa oluşturulmaz.** Varsayılanlarla çalışır. Dosya ilk kez
  "Ayar dosyasını aç" basıldığında veya menüden bir ayar değiştirildiğinde yazılır.

### `monitor` neden sadece iki değer

Ekran indeksi (`"monitor": 2`) **bilinçli olarak yok**: monitör enumerasyonu +
tepsi menüsünde anlamlı adlandırma + monitör takılıp çıkarılınca indeksin
kayması ayrı bir iş. `primary` ve `cursor` gerçek ihtiyacı karşılıyor. Gerekirse v2.

### Frontend yarış durumu

WebView, backend'in `config` emit'inden **sonra** yüklenirse temayı kaçırır
(P3'te aynı sorun `Frame` için yaşandı). Çözüm: `#[tauri::command] fn ayarlar()`
— frontend init sırasında ayarı **kendisi çeker**; `config` emit'i yalnızca
sonraki **değişiklikler** için kullanılır. `src/main.ts`'teki `VARSAYILAN_TEMA`
sabiti bu çağrıyla değişir (P5 notu orada zaten duruyor).

---

## 4. Tepsi ikonu

```
┌─ neon-flyout ──────────┐
│ Tema      ▸ ● cyber    │
│              ○ neon    │
│ Köşe      ▸ ● sağ üst  │
│              ○ sağ alt │
│              ○ sol üst │
│              ○ sol alt │
│ ─────────────────────  │
│ ☑ Windows ile başlat   │
│ Ayar dosyasını aç      │
│ Ayarları yeniden yükle │
│ ─────────────────────  │
│ Çıkış                  │
└────────────────────────┘
```

- `tauri::tray::TrayIconBuilder` + `tauri::menu::{Menu, Submenu, CheckMenuItem}`.
  `Cargo.toml`'da `tauri` için **`tray-icon` feature'ı** açılacak.
- Menüden yapılan her değişiklik: (1) bellekteki Config'i günceller, (2) ilgili
  tüketiciye yollar, (3) `config.json`'a yazar. Sıra bu — yazma başarısız olsa
  bile değişiklik o oturumda geçerli olur.
- **"Ayarları yeniden yükle"** dosyayı elle düzenleyenler için. Dosya izleme
  (`notify` crate) **eklenmiyor**: bir bağımlılık ve bir thread daha, karşılığı
  bir menü tıklaması.
- **İkon:** `app.default_window_icon()` → `icons/icon.ico`. Bu **Tauri'nin
  varsayılan logosu**; projeye ait bir ikon henüz yok. P5 kapsamında ikon
  tasarlanmıyor, ayrı bir iş olarak bırakılıyor.
- Sol tıklama davranışı **yok** (menü sağ tıklamada). Tıklanınca gösterilecek bir
  pencere yok; kutu olaya tepki verir, isteğe değil.

---

## 5. Windows ile başlat — Başlangıç klasörü kısayolu

`adapters/autostart.rs`, üç işlev: `enabled() -> bool`, `enable()`, `disable()`.

- Hedef: `SHGetKnownFolderPath(FOLDERID_Startup)` → `neon-flyout.lnk`
- Yazma: COM `IShellLinkW` (`SetPath` + `SetWorkingDirectory`) → `IPersistFile::Save`
- **Gerçek kaynak dosyanın kendisidir**, `config.json` değil: `enabled()`
  kısayolun varlığına bakar. Kullanıcı klasörden elle silerse menü doğruyu gösterir.
  `config.json`'daki `autostart` alanı yalnızca bilgi amaçlı yazılır ve
  okunduğunda **dosya durumu kazanır**.
- `Cargo.toml` → `windows` crate'ine `Win32_UI_Shell` feature'ı eklenecek
  (`Win32_System_Com` zaten var).
- **Dev'de kısayol debug exe'sini gösterir.** Beklenen davranış; koda not düşülecek.

---

## 6. P4'ten devir: düzgün çıkış — ve bir hata düzeltmesi

**Zincir:** tepsi `Çıkış` → `app.exit(0)` → `RunEvent::Exit` → `restore()`.
`lib.rs`'te `RunEvent::Exit` kancası zaten var; eksik olan tek şey çıkışı
tetikleyecek bir yoldu.

**Kod okurken bulunan hata — P5 kapsamına alınıyor.**
`adapters/native_flyout.rs:89`, `suppress()` o anki ex-style'ı kaydediyor:

```rust
let ex_style = GetWindowLongPtrW(hwnd(host), GWL_EXSTYLE);
// ...
*g = Some(Suppressed { host, ex_style });
```

Süreç öldürülüp (kill-9) uygulama yeniden başlarsa kabuğun penceresi **hâlâ
bastırılmış** durumdadır; dolayısıyla kaydedilen stil `WS_EX_LAYERED` ve
`WS_EX_TRANSPARENT` bitlerini **içerir**. Sonraki düzgün çıkışta `restore()`
"bastırılmış stili" geri yükler ve pencere görünmez kalır.

**Düzeltme:** kaydetmeden önce bu iki biti maskele:

```rust
let ex_style = GetWindowLongPtrW(hwnd(host), GWL_EXSTYLE)
    & !(WS_EX_LAYERED.0 as isize | WS_EX_TRANSPARENT.0 as isize);
```

`restore()` zaten pencereyi `0,0`'a alıp `SW_RESTORE` yaptığı için, bu maskeyle
birlikte kill-9 sonrası ilk düzgün çıkış durumu **tam olarak** düzeltir. Kurtarma
betiği (`spikes/p4-probe/p4-restore.ps1`) son çare olarak yerinde kalır.

**Doğrulanacak varsayım:** bu düzeltme, kabuğun penceresinin özgün stilinde bu iki
bitin **bulunmadığı** varsayımına dayanıyor. Uygulama sırasında bastırılmamış bir
sistemde `GetWindowLongPtrW` okunarak ölçülecek; bit zaten varsa maskeleme yanlış
olur ve bu bölüm revize edilir (o durumda "ilk görülen stili kalıcı yaz" yoluna
geçilir).

---

## 7. Installer

`app/src-tauri/tauri.conf.json`:

```json
"bundle": {
  "active": true,
  "targets": ["nsis"],
  "icon": [ "...mevcut liste..." ],
  "windows": {
    "webviewInstallMode": { "type": "embedBootstrapper" },
    "nsis": { "installMode": "currentUser" }
  }
}
```

- `embedBootstrapper`: kurulum exe'si küçük kalır; WebView2 yoksa Microsoft'un
  bootstrapper'ı çalışıp indirir (internet gerekir). Win11'de runtime zaten
  kurulu olduğu için pratikte tetiklenmez.
- `currentUser`: UAC yok, tek kullanıcı için kurulum — tepsi uygulamasına uygun.

### İki risk, ikisi de erken denenecek

1. **Frontend varlıkları.** `src/main.ts` `../../design/*.css` dosyalarını import
   ediyor, onlar da `design/fonts/*.woff2`'ye `@font-face` ile bağlı.
   **`npm run build`'in bunları `dist/`e doğru taşıdığı dev'de kanıtlanmış
   değildir.** Uygulamanın ilk adımı bu — taşımıyorsa kutu kurulumda fontsuz çıkar.
2. **İmzasız installer.** Kod imzalama sertifikası yok → SmartScreen
   "bilinmeyen yayıncı" uyarısı verir. Sertifika alınmadan kaçınılmaz; kabul
   edilen bir sonuç olarak kayda geçiyor.

---

## 8. Çıkış kriteri

ROADMAP "temiz bir Windows kurulumunda çift tıkla çalışıyor" diyor. Temiz makine
(VM) elde yoksa erişim dışında; bu yüzden ölçülebilir kriter şu — **hepsi kurulu
exe üzerinde, `npm run tauri dev` ile değil**:

1. Installer kurulumu tamamlanıyor, uygulama Başlat menüsünden açılıyor.
2. Tepsi ikonu görünüyor; menü açılıyor.
3. Tema menüden değiştirilince **bir sonraki kutu** yeni temada çıkıyor.
4. Köşe menüden değiştirilince **bir sonraki kutu** yeni köşede çıkıyor.
5. `%APPDATA%\neon-flyout\config.json` yazılıyor; elle düzenlenip "Ayarları
   yeniden yükle" denince değişiklik geçerli oluyor.
6. Bozuk JSON yazılınca uygulama çökmüyor, varsayılanlarla çalışıyor, dosyanın
   üstüne yazmıyor.
7. "Windows ile başlat" işaretlenince Başlangıç klasöründe kısayol oluşuyor,
   kaldırılınca siliniyor.
8. `Çıkış` sonrası **Windows'un kendi medya flyout'u geri geliyor**.
9. Uygulama Görev Yöneticisi'nden öldürülüp yeniden başlatılıyor, sonra düzgün
   çıkılıyor → kabuğun kutusu yine geri geliyor (§ 6 düzeltmesi).
10. Kaldırma tamamlanıyor; `config.json` **kalıyor** (kullanıcı verisi silinmez).
11. `cargo test` geçiyor — mevcut 16 test + `core/config.rs`'in yeni testleri.

VM varsa 1–11 temiz kurulumda tekrarlanır.

---

## 9. P5'te yapılmayacaklar

- Ayar penceresi (ikinci WebView).
- Projeye özel ikon tasarımı.
- Kod imzalama sertifikası.
- Ayar dosyası izleme (`notify`) — menüde "yeniden yükle" var.
- Ekran indeksiyle monitör seçimi.
- MSIX paketleme. *(Yapılırsa `globalMediaControl` capability sorusu geri gelir —
  `docs/P1-SONUC.md`.)*
- Otomatik güncelleme (`tauri-plugin-updater`).
