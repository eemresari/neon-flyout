# P5 — Paketleme: sonuç

**Tarih:** 2026-08-30
**Tasarım:** `docs/superpowers/specs/2026-08-30-p5-paketleme-design.md`
**Plan:** `docs/superpowers/plans/2026-08-30-p5-paketleme.md`

Uygulama artık kurulabilir bir tepsi programı: ayar dosyası, tepsi menüsü,
Windows ile başlat, NSIS installer. P4'ün açık bıraktığı "düzgün çıkış" da
burada kapandı — ve o yolda ikinci bir hata bulundu (§ 3).

---

## 1. Frontend varlık kapısı — `design/` build yolunda da çözülüyor

**Sonuç: kapı açık, `vite.config.ts` değişikliği gerekmedi.**

Endişe gerçekti: `src/main.ts` `../../design/*.css` import ediyor, `design/`
Vite kökünün (`app/`) **dışında**, ve `vite.config.ts`'teki `server.fs.allow`
yalnızca **dev sunucusu** için geçerli — build yolunu kanıtlamıyor. Kurulumda
fontsuz çıkan bir kutuyu ancak installer'ı kurduktan sonra fark ederdik, bu
yüzden ilk iş bu ölçüldü.

`npm run build` çıktısı: 16 woff2 `dist/assets/` altına kopyalandı, CSS'teki
yollar `/assets/...woff2` biçiminde **dist içine** yeniden yazıldı, hiçbir
`url()` depo yoluna kaçmadı. Vite CSS `url()`'lerini kök dışı olsalar da
topluyor.

### Yan bulgu — JetBrains Mono 500 aslında 400'ün kopyası

`design/fonts/` içinde 18 woff2 var ama `dist/`e 16 tane çıktı. Sebep bir build
hatası **değil**: `dist` CSS'inde 18 `@font-face` bloğunun hepsi duruyor, ama
JetBrains 500 blokları 400'ün dosyasını gösteriyor. Vite içerik hash'iyle
tekilleştirmiş — çünkü dosyalar birebir aynı:

```
jetbrains-mono-400-latin-ext.woff2   11624 B   MD5 BD4781A31B42
jetbrains-mono-500-latin-ext.woff2   11624 B   MD5 BD4781A31B42
jetbrains-mono-400-latin.woff2       31432 B   MD5 B636A65DA4F0
jetbrains-mono-500-latin.woff2       31432 B   MD5 B636A65DA4F0
```

Yani P3'te fontlar indirilirken JetBrains Mono'nun 500 ağırlığı için 400 verisi
alınmış. Etkisi: `cyber` temasının `--font-mono` metni 500 istendiği yerde 400
glifleriyle çiziliyor — tasarlanandan bir tık ince. Sentetik kalınlaştırma
olmuyor, çünkü `@font-face` 500 diye bildiriyor.

**Dev'de de aynı, P5'in getirdiği bir gerileme değil.** Düzeltmesi `design/`
altına Google Fonts'tan doğru dosyaları indirmek; P5 kapsamına alınmadı.

---

## 2. Mimari — ayar tüketicisine göre bölündü

Tek bir paylaşılan config nesnesinden okuma yok. Yetkili kopya
`SharedConfig(Mutex<Config>)` app state'inde duruyor, ama oradan **sürekli
okunmuyor** — değiştiğinde üç tüketiciye ayrı ayrı **itiliyor**:

```
                       ┌─ theme ───────────→ app.emit("config") ──→ WebView
config.json ─┐         │
             ├─ Config ┼─ corner, monitor ──→ window::set_placement()
tepsi menüsü ┘         │
                       └─ show_ms, fade_ms,  ─→ mpsc ──→ DomainEvent::ConfigChanged
                          coalesce_ms,               (core döngüsü uygular)
                          skip_when_foreground
```

**Kritik kısıt korundu:** `Engine`'e yalnızca core thread'i dokunuyor. Zamanlama
ve kural ayarları `Arc<RwLock<..>>` ile paylaşılmıyor, mevcut mpsc kanalından
`DomainEvent::ConfigChanged(EngineConfig)` olarak geçiyor — P2'nin tek-yazıcı
disiplini (`docs/P2-SONUC.md`) bozulmadı.

`ConfigChanged` bir **tetik değil**: `Engine::handle` onu ilk satırda yakalayıp
efekt üretmeden dönüyor. Üç test bunu kodluyor: kutuyu açmaması, yeni `show_ms`'in
bir sonraki tetikte geçerli olması, ve gürültü kurallarının "görülmüşlük"
geçmişini **silmemesi** (silseydi her ayar değişiminden sonra bir olay
"ilk olayı yut" kuralına yem olurdu).

### Ayar dosyası

`%APPDATA%\neon-flyout\config.json`. Tauri'nin `app_config_dir()` fonksiyonu
bundle identifier kullanıp `%APPDATA%\dev.neonflyout.app` verirdi — elle
düzenlenecek bir dosya için okunaksız, o yüzden `%APPDATA%` doğrudan okunuyor.

Dayanıklılık kuralları ve hepsinin testi `core/config.rs`'te:

- eksik alan → varsayılan; tanınmayan alan → yok sayılır
- **bozuk JSON → tamamı varsayılan, dosyanın üstüne yazılmaz** (kullanıcının
  yazdığı kaybolmasın)
- aralık dışı sayı → kırpılır (`show_ms` 500–15000, `fade_ms` 0–2000,
  `coalesce_ms` 0–5000)
- **tanınmayan enum değeri yalnızca o alanı düşürür**, tüm dosyayı değil.
  Serde varsayılan davranışı `"theme": "mor"` gördüğünde bütün nesnenin
  ayrıştırmasını hataya düşürüyor; alan başına `deserialize_with = "esnek"`
  ile `serde_json::Value` üzerinden kurtarılıyor. Kullanıcı bir harf yanlış
  yazınca bütün ayarını kaybetmemeli.
- varsayılanlar `visibility.rs` ve `rules.rs` sabitlerinden okunuyor, burada
  yeniden yazılmıyor — bir test bu eşitliği koruyor.

### Frontend yarış durumu

WebView backend'in emit'inden sonra yüklenirse temayı kaçırırdı (P3'te `Frame`
için aynısı yaşanmıştı). Çözüm: `#[tauri::command] ayarlar()` — frontend init'te
ayarı **kendisi çekiyor**, `config` emit'i yalnızca sonraki değişiklikler için.
`VARSAYILAN_TEMA` senkron uygulanıp üstüne yazılıyor, böylece kutu hiçbir anda
temasız boyanmıyor.

---

## 3. P4'ten devir: düzgün çıkış — ve kaydedilen ex-style hatası

Düzgün çıkış tepsi menüsüyle geldi: `Çıkış` → `app.exit(0)` → `RunEvent::Exit`
→ `native_flyout::restore()`. `RunEvent::Exit` kancası P4'ten beri duruyordu;
eksik olan tek şey onu tetikleyecek bir yoldu.

**Kod okurken ikinci bir hata bulundu.** `suppress()` o anki ex-style'ı
kaydediyordu:

```rust
let ex_style = GetWindowLongPtrW(hwnd(host), GWL_EXSTYLE);
```

Süreç öldürülüp (kill-9) uygulama yeniden başladığında kabuğun penceresi **hâlâ
bastırılmış** durumdadır — dolayısıyla kaydedilen stil `WS_EX_LAYERED` ve
`WS_EX_TRANSPARENT` bitlerini içeriyordu, ve sonraki **düzgün** çıkışta
`restore()` "bastırılmış stili" geri yazıyordu. Yani kill-9'dan sonraki ilk
düzgün çıkış pencereyi kurtarmıyordu.

Düzeltme, kaydetmeden önce iki biti maskelemek. Ama önce varsayım ölçüldü:
**özgün stilde bu bitler var mı?** Bastırılmamış sistemde, `FindWindowEx` ile
(§ 3.5 — `EnumWindows` band 18'i görmüyor):

```
hwnd=526796   proc=explorer  exstyle=0x08200088  LAYERED=False TRANSPARENT=False
hwnd=1312314  proc=explorer  exstyle=0x00200088  LAYERED=False TRANSPARENT=False
```

`0x08200088` = `WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOREDIRECTIONBITMAP |
WS_EX_NOACTIVATE`. İkisinde de bit yok → maskeleme güvenli, uygulandı.

Kurtarma betiği (`spikes/p4-probe/p4-restore.ps1`) son çare olarak yerinde
duruyor.

---

## 4. Tepsi menüsü

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

**Tauri v2 menüsünde radyo öğesi yok.** Tema ve köşe onay işaretli
(`CheckMenuItem`) öğelerle gösteriliyor, karşılıklı dışlama elle yapılıyor:
tıklanan öğeyi Windows zaten çeviriyor, `isaretle()` diğerlerini söndürüyor.

Menü değişiminde sıra: (1) yetkili Config güncellenir, (2) tüketiciye itilir,
(3) dosyaya yazılır. Yazma başarısız olsa bile değişiklik o oturumda geçerli olur.

**Dosya izleme yok.** `notify` crate'i bir bağımlılık ve bir thread daha
demekti; karşılığı bir menü tıklaması ("Ayarları yeniden yükle").

**İkon Tauri'nin varsayılan logosu** — projeye ait bir ikon henüz yok, P5
kapsamına alınmadı.

### "Windows ile başlat"

Başlangıç klasörüne `IShellLinkW` + `IPersistFile` ile `.lnk` yazılıyor
(`SHGetKnownFolderPath(FOLDERID_Startup)` — sabit yol değil, yerelleştirilmiş
kurulumlar için). **Gerçek kaynak dosyanın kendisi**, `config.json` değil:
kullanıcı kısayolu klasörden elle silerse menü doğruyu gösterir.

COM yolu `#[ignore]`'lı bir testle uçtan uca doğrulandı — yaz, gör, sil, iz
bırakma. Varsayılan takımda çalışmaz (gerçek Başlangıç klasörüne yazıyor):

```
cargo test --lib autostart -- --ignored --nocapture
```

### E0716 tuzağı

`app.state::<SharedConfig>().0.lock()` zincirini doğrudan bir `let`'e bağlamak
derlenmiyor — `state` guard'ı geçici ve kilit hâlâ kullanılırken düşüyor. Guard
önce bir değişkene bağlanmalı. Kopyalayarak okuyan (`let cfg = *...`) kullanım
sorunsuz, çünkü deref-kopya ifade içinde bitiyor.

---

## 5. Installer

NSIS, `webviewInstallMode: embedBootstrapper`, `installMode: currentUser`
(UAC yok). WebView2 yoksa Microsoft'un bootstrapper'ı indirir; Win11'de runtime
zaten kurulu, pratikte tetiklenmiyor.

**İmzasız** — kod imzalama sertifikası yok, SmartScreen "bilinmeyen yayıncı"
uyarısı veriyor. Sertifika alınmadan kaçınılmaz.

Boyutlar (`npm run tauri build`, 2 dk):

| | |
|---|---|
| `neon-flyout_0.1.0_x64-setup.exe` | **3,66 MB** |
| `neon-flyout.exe` (ham) | 8,82 MB |

Tauri build sırasında NSIS 3.11'i, `nsis_tauri_utils.dll`'i ve WebView2
bootstrapper'ını (`go.microsoft.com/fwlink/p/?LinkId=2124703`) kendisi indirdi —
**ilk build internet gerektiriyor**, sonrakiler önbellekten.

<!-- DOLDURULACAK: kurulum/kaldırma davranışı -->

---

## 6. Çıkış kriteri

<!-- DOLDURULACAK: 12 maddenin sonucu, kurulu exe üzerinde -->

---

## 7. Test durumu

`cargo test` → **34 geçiyor, 1 ignored**. Taban 16'ydı; P5 18 test ekledi:

| Nerede | Adet | Ne koruyor |
|---|---|---|
| `core/config.rs` | 9 | serde varsayılanları, bozuk JSON, enum kurtarma, kırpma, `EngineConfig` türetme, varsayılanların sabitlerle eşitliği |
| `core/place.rs` | 6 | dört köşe, negatif koordinatlı ikincil monitör, kutu ekrandan büyükse taşmama |
| `core/engine.rs` | 3 | `ConfigChanged` tetik değil, yeni `show_ms` sonraki tetikte, gürültü geçmişi silinmiyor |
| `adapters/autostart.rs` | 1 (ignored) | kısayol yaz/gör/sil turu — gerçek Başlangıç klasörüne yazdığı için varsayılan takımda kapalı |

---

## 8. P5'te yapılmayanlar

- Ayar penceresi (ikinci WebView) — tepsi menüsü + JSON yeterli görüldü.
- Projeye özel ikon.
- Kod imzalama sertifikası.
- Ayar dosyası izleme (`notify`).
- Ekran indeksiyle monitör seçimi — `primary` ve `cursor` var; indeks
  enumerasyon + adlandırma + monitör takılınca kayma sorunları getiriyor.
- MSIX paketleme. *(Yapılırsa `globalMediaControl` capability sorusu geri gelir
  — `docs/P1-SONUC.md`.)*
- Otomatik güncelleme (`tauri-plugin-updater`).
- **JetBrains Mono 500 fontunun düzeltilmesi** (§ 1 yan bulgu).
