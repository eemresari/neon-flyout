# P3 — arayüz sonucu

**Tarih:** 2026-08-28 · **Durum: tamamlandı.** `design/` altındaki CSS uygulamada
çalışıyor, sekiz durum `design/preview.html` ile örtüşüyor, WebView2 gizliyken
render'ı duruyor ve P0 ölçümü tekrarlandı.

---

## 1. Tasarım nasıl bağlandı — kopyalanmadı, import edildi

`design/*.css` üretim kodu (`docs/DESIGN.md`). Frontend onları **doğrudan** çekiyor:

```ts
import "../../design/fonts.css";
import "../../design/flyout.css";
import "../../design/themes/cyber.css";
import "../../design/themes/neon.css";
import "./styles.css";          // yalnızca pencere kabuğu
```

`design/` Vite kökünün (`app/`) dışında kaldığı için `vite.config.ts` →
`server.fs.allow: [".", ".."]` gerekti. Karşılığı: temayı değiştirmek için tek
dosya var, "önce design'da düzelt sonra app'e taşı" adımı hiç yok.

`app/src/styles.css` artık **sadece pencereye** dair: `background: transparent`,
`overflow: hidden`, kutunun mutlak konumu. Kutunun görünüşüne ait tek bir değer
bile burada değil — bulursan yanlış yere yazılmıştır.

### Fontlar artık yerel

`design/fonts.css` + `design/fonts/*.woff2` (20 dosya, ~250 KB). Google Fonts CDN
linki kaldırıldı; uygulama offline açılıyor. Yalnızca `latin` + `latin-ext` alt
kümeleri alındı (arayüz Türkçe; ı, ş, ğ latin-ext'te). `font-display: block`
seçildi: kutu 2.5 saniye duruyor, yedek fontla açılıp gerçek fonta atlamak o süre
içinde göze zıplama olarak görünürdü.

`design/preview.html` de aynı yerel dosyaları kullanıyor — önizleme ile uygulama
arasında font farkı kalmadı.

---

## 2. `Frame` → DOM

Sözleşme P2'den değişmedi (`docs/P2-SONUC.md` § 2). DOM `docs/DESIGN.md`'deki
iskeletin birebir aynısı; **iki kutu da DOM'da duruyor**, kullanılmayanı tema
`display: none` ile kapatıyor, tema değişimi tek attribute.

Yazarken dikkat edilenler:

- **`phase` animasyonu yönetiyor, gösterimi değil.** `enter` → `.is-visible`
  eklenir (150 ms CSS geçişi); `update` → sadece metin/değer yazılır, sınıfa
  dokunulmaz; `leave` → `.is-visible` kaldırılır. Parça geçişinde başlık 270 ms
  sonra geldiği için `update` sık geliyor; animasyonun yeniden oynamaması bu
  ayrımın tamamen doğru olmasına bağlı.
- **Medya ↔ ses geçişi animasyonsuz.** Pencere zaten ekranda; eski kutu `hidden`
  yapılıp yenisi geçişsiz açılıyor, yoksa iki kutu üst üste biniyor.
- **`hidden` kalktığı kare ile `.is-visible` eklendiği kare aynı olamaz** —
  tarayıcı ikisini birleştirir ve giriş animasyonu hiç oynamaz. Araya
  `void kutu.offsetWidth` (zorlanmış reflow) kondu.
- **`position` null gelebilir.** İnterpolasyon **yok**: ölçüm `ageMs` kadar
  bayat, çalarken o kadar ilerlemiş sayılıyor, duraklatılmışken sayılmıyor, tek
  okuma yazılıyor. Kutu 2.5 saniye duruyor; saniyede ilerleyen bir sayaç için
  sürekli kare üretmenin karşılığı yok.
- **Kaynak adı kısaltılıyor:** `SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify` →
  `Spotify`. AUMID olduğu gibi yazılınca kutuya sığmıyor.

---

## 3. Kapak görseli — gizliyken çözülüyor

SMTC kapağı `TryGetMediaPropertiesAsync().Thumbnail()` ile ham bayt olarak
geliyor; `data:image/...;base64,...` olarak `MediaSnapshot.art` alanında
WebView'e gidiyor.

İki karar:

1. **Parça başına bir kez çözülüyor, olay başına değil.** Anahtar
   `(sourceId, title, artist, album)`; tek kayıtlık cache yetiyor, aynı anda tek
   parça çalıyor. "Kapak yok" sonucu da cache'leniyor, yoksa 811 ms'ye yayılan
   sekiz olayın her birinde WinRT'ye boşuna gidilir.
2. **Olayların çoğu kutu gizliyken geliyor**, dolayısıyla çözme işi zaten
   gösterimden önce bitmiş oluyor — ilk boyanan karede görsel hazır.

MIME `ContentType`'tan değil **sihirli baytlardan** okunuyor; bazı kaynaklarda
`ContentType` boş geliyor. Üst sınır 512 KB: bozuk bir kaynak IPC'yi tıkamasın.
Kapak yoksa `img` gizleniyor ve temanın `--art-fallback` gradyanı görünüyor.

---

## 4. Tepe metresi — core'dan geçmiyor, bilerek

Cyber'ın dikey segment metresi (`.fly__meter`) ve neon'un yatay spektrumu
(`.fly__spectrum`) aynı girdiyi kullanıyor: WASAPI
`IAudioMeterInformation::GetPeakValue`, ~30 Hz.

Bu akış **ayrı bir kanalda** (`peak`) ve `core`'a hiç uğramıyor. Sebep: tetik
değil, deadline'ı uzatmıyor, durumun parçası değil. `flyout` kanalına konsaydı
saniyede 30 kez tam `PlayerState` — kapak base64'ü dahil — yeniden gönderilirdi.

Metre kutunun ömrüyle birebir: `Effect::Show` ile başlıyor, `Effect::Hide` ile
duruyor. Gizliyken tek örnek bile alınmıyor, thread `recv()`te süresiz uyuyor.
Uç nokta her `Start`'ta yeniden çözülüyor — cihaz değiştiyse bir sonraki
gösterimde yenisi bulunuyor, ayrıca `IMMNotificationClient` kurmaya gerek yok.

---

## 5. WebView2'nin gizliyken render'ı — P0 ölçümü tekrarlandı

P0'ın açık bıraktığı soru buydu: `SW_HIDE` render döngüsünü durdurmuyordu
(gizliyken saniyede 185 kare), dolayısıyla ölçülen 11 ms **sıcak** bir döngüden
alınmıştı ve "asıl uyanma bedeli" bilinmiyordu.

Ölçüm düzeneği aynı (`spikes/p0-tauri`, `screen_ms` = masaüstü pikselinin
değiştiği an), iki yeni gizleme yolu eklendi.

| Koşu | n | p50 | **p95** | max | gizliyken üretilen kare |
|---|---:|---:|---:|---:|---:|
| `tauri-hide-short` *(P0, ham `SW_HIDE`)* | 25 | 11.1 | **12.1** | 12.4 | 9 235 |
| `tauri-hide-short-suspend` | 25 | 11.4 | **11.6** | 11.7 | **0** |
| `tauri-hide-short-trysuspend` | 25 | 11.4 | **11.8** | 12.1 | **0** |
| `tauri-hide-long` *(P0, 60 sn bekleme)* | 5 | 11.3 | **12.0** | 12.0 | 11 098 |
| `tauri-hide-long-suspend` *(60 sn)* | 5 | 11.4 | **11.8** | 11.9 | **0** |

Sayılar ms. Ham veri `measurements/tauri-*-suspend.csv` (git'e girmiyor).

### Okunacak üç şey

1. **Uyanma bedeli yok.** Korkulan senaryo — "render durdurulunca ilk kare
   pahalılaşır" — **çıkmadı**. `SetIsVisible(false)` ham `SW_HIDE`'dan
   ölçülebilir biçimde *daha iyi* (p95 11.6 vs 12.1); fark bir karenin (5.43 ms)
   altında, yani pratikte "aynı".
2. **Render gerçekten duruyor.** 55 tekrarın 55'inde `idle_ticks = 0`. GPU'nun
   boşuna dönmesi bitti; 60 saniyelik profilde de sıfır.
3. **60 saniye gizli kalmak hâlâ bedava** (11.8 vs 11.6). P0'ın "derin bekleme ek
   maliyet getirmiyor" bulgusu, döngü artık gerçekten durduğunda da geçerli.

### Neden `SetIsVisible(false)`, `TrySuspend` değil

Gecikme farkı yok (11.6 vs 11.8, ikisi de gürültü içinde). Ayıran şey davranış:

`TrySuspend` **JavaScript'i de donduruyor.** Bizim akışta içerik pencereden ÖNCE
gidiyor — `apply()` sırası `Effect::Emit` → `Effect::Show`, ki ilk boyanan kare
doğru içeriği taşısın (P2'nin kararı). Donmuş bir WebView o olayı ancak
`Resume`'dan sonra işler; `Resume` ise gösterme yolunun içinde, dolayısıyla ilk
kare bayat içerikle çıkabilir. Kazanç bellek, gecikme değil — **P5'te paketleme
sırasında ayrıca değerlendirilebilir**, şimdi gereksiz risk.

Uygulamadaki karşılığı `app/src-tauri/src/adapters/webview.rs`. Sıra önemli:
gösterirken **önce WebView uyanır sonra pencere görünür**, gizlerken **önce
pencere gizlenir sonra WebView durur** — tersi ilk/son karede boşluk bırakıyor.

---

## 6. Ölçüm düzeneğinde düzeltilen tuzak

`spikes/p0-tauri` release ikilisi `windows_subsystem = "windows"` ile derleniyor:
kabuk süreci beklemiyor, komut anında dönüyor. Üç profili tek satırda arka arkaya
çağırmak üç koşuyu **aynı anda** başlatıyor; pencereler üst üste biniyor ve
piksel yoklaması ya `GECERSIZ` diyor ya `NaN` üretiyor.

Koşuları `Start-Process -Wait` ile sırayla çalıştır. Kalibrasyon satırındaki
`GECERLI` ve `idle_ticks` sütunu bu hatayı yakalıyor — koşuyu okurken ikisine de
bak.

---

## 7. P3'ten devredilenler

- **Tema seçimi hâlâ derleme zamanında** (`main.ts` → `VARSAYILAN_TEMA`). Çalışma
  anında değiştirmek için `__tema("neon")` var ama kalıcı ayar **P5'in işi**
  (ayar dosyası). Aynısı köşe/monitör seçimi için de geçerli; şu an kutu birincil
  monitörün sağ üst köşesinde, 16 px boşlukla.
- **AudioAdapter'ın cihaz değişimi yolu gerçek donanımla hâlâ denenmedi**
  (`IMMNotificationClient` → yeniden bağlanma). Tepe metresi bu yola bağlı değil
  — o her gösterimde uç noktayı yeniden çözüyor — ama ses seviyesi callback'i
  bağlı. Kulaklık takıp çıkararak doğrulanmalı.
- `TrySuspend` (bellek kazancı) — yukarıdaki gerekçeyle ertelendi.
