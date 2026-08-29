# Yol haritası

Her aşamanın bir çıkış kriteri var. Kriter sağlanmadan sonraki aşamaya geçme.

---

## P0 — Gecikme kapısı ✅ TAMAMLANDI (2026-08-28) — GEÇTİ

**Sonuç: Tauri p95 12.9 ms / bütçe 50 ms → yığın Tauri v2 + Rust.** Ölçüm yöntemi,
sayılar ve yan bulgular: **`docs/P0-SONUC.md`**. Aşağıdaki plan tarihsel kayıt olarak
duruyor; iki noktada uygulamadan ayrıldı:

- rAF ile ilk kareyi yakalama yöntemi **geçersiz çıktı** (render döngüsü pencere
  gizliyken durmuyor). Yerine masaüstü pikselini yoklayan, çerçeveden bağımsız yöntem
  kullanıldı.
- Yol haritasında olmayan iki profil eklendi: `hide-long` (60 sn derin bekleme) ve
  `offscreen-short` (alternatif gizleme). İkisi de karara katkı sağladı.

**Bu aşamayı tekrar çalıştırma; P1'den devam et.**

<details>
<summary>Özgün P0 planı</summary>

**Bu bir ölçüm, uygulama değil.** UI yok, tasarım yok, SMTC yok.

Yapılacak:

1. Boş bir Tauri v2 penceresi: `transparent: true`, `decorations: false`,
   `alwaysOnTop: true`, `skipTaskbar: true`, `focus: false`.
2. Pencereyi `SW_HIDE` ile gizle, bir tuşla `SW_SHOW` yap.
3. `QueryPerformanceCounter` ile **tetik anı → ilk boyanan kare** arasını ölç.
   İlk kareyi WebView tarafından `requestAnimationFrame` içinde bir IPC ile geri
   bildirerek yakala, ya da harici bir yüksek FPS kayıt ile doğrula.
4. En az 20 tekrar, p50 ve p95 raporla.
5. **Aynı testi bir WPF penceresiyle tekrarla** ve karşılaştır.

**Kapı:** Tauri p95 < 50 ms ise devam. Değilse plan aynı kalır, yığın WPF'e döner.
Sonucu `CLAUDE.md`'deki varsayım listesine yaz.

İkinci soru, aynı anda ölçülebilir: şeffaf pencerede `backdrop-filter: blur()`
Windows'ta çalışıyor mu?

</details>

---

## P1 — SMTC spike ✅ TAMAMLANDI (2026-08-28) — GEÇTİ

**Çıkış kriteri sağlandı:** Spotify ↔ Opera GX ve Spotify ↔ Edge geçişlerinde doğru
metadata aktı. Capability sorunu **yok**. Düzenek: `spikes/p1-smtc`, sonuçlar ve olay
akışının ham şekli: **`docs/P1-SONUC.md`**.

P2'yi doğrudan değiştiren üç bulgu:

- **Yeni oturum boş doğuyor.** `CurrentSessionChanged` geldiğinde başlık boş, durum
  `Opened`; içerik 14 ms sonra `MediaPropertiesChanged` ile geliyor. Bu olay tek
  başına kutuyu **açmamalı**.
- **150 ms birleştirme penceresi yetersiz.** Tek parça geçişi 811 ms'ye yayılan 8 olay
  üretiyor, başlık 270. ms'de geliyor. Pencere "kutuyu aç" kararını değil,
  "animasyonu baştan oynatma" kararını yönetmeli.
- **Olaylar eşzamanlı thread'lerde geliyor.** Adapter durum tutmamalı; olay domain
  tipine çevrilip mpsc'ye yazılmalı.

**Bu aşamayı tekrar çalıştırma; P2'den devam et.**

<details>
<summary>Özgün P1 planı</summary>

Tauri yok, UI yok. Sadece bir Rust konsol uygulaması.

- Aktif oturumu bul, başlık / sanatçı / durum / pozisyon bas
- `CurrentSessionChanged` ile oturum değişimini izle
- `TryTogglePlayPauseAsync` ile bir komut gönder
- Paketlenmemiş exe'de `globalMediaControl` capability sorunu çıkıyor mu, not et

**Çıkış:** Spotify ↔ tarayıcı sekmesi geçişinde doğru metadata akıyor.

</details>

---

## P2 — Tetik + zamanlayıcı ✅ TAMAMLANDI (2026-08-28)

**Uygulama artık repo kökündeki `app/` altında; `spikes/` burada bitti.** Katmanlar,
core → WebView sözleşmesi, kararlar ve tuzaklar: **`docs/P2-SONUC.md`**.

- **Çıkış kriteri 5 sağlandı:** `cargo test` → **16/16**. Gürültü kuralları ve
  görünürlük durum makinesi birim testli; testler kriterlerin kendisini kodluyor
  (10 art arda tetik → tek kutu, P1'in 811 ms'lik serisi → tek açılış + yerinde
  güncelleme, boş oturum → hiçbir efekt).
- **Kriter 1–4 kısmen doğrulandı.** Kutu doğru anda beliriyor, kaynak ve çalma
  durumu doğru yazılıyor, süre dolunca kayboluyor. **Ses tuşuna arka arkaya basma**
  ve **Spotify penceresinden fareyle durdurma** senaryoları ayrıca teyit edilmedi —
  P3'ün ilk işi, tasarım bağlanmadan önce (sonra bir aksaklığın hangi katmandan
  geldiğini ayırt etmek zorlaşır).
- **Çalıştırma: `cd app && npm run tauri dev`.** `cargo run` tek başına çalışmaz —
  debug derlemesi `devUrl`'e gider, Vite ayakta değilse WebView "localhost
  bağlantıyı reddetti" gösterir.

<details>
<summary>Özgün P2 planı</summary>


Artık gerçek uygulama; `spikes/` burada biter. İçerik hâlâ çirkin olabilir — bu aşamada
tasarım bağlanmıyor, `design/` P3'ün işi. Amaç: **doğru anda beliren, doğru anda
kaybolan, titremeyen bir kutu.**

### P2.0 — İskeleti kur (henüz yok)

Uygulama kodu **repo kökündeki `app/`** altında yaşar (`spikes/` atılacak koddan
ayrı dursun diye).

```bash
npm create tauri-app@latest      # elle uydurma; ad: neon-flyout, dizin: app
```

Şablon **vanilla + TypeScript**: `design/` düz HTML/CSS, araya framework sokmanın
karşılığı yok.

Pencere ayarları P0'da doğrulanan haliyle (`tauri.conf.json`):
`transparent: true`, `decorations: false`, `alwaysOnTop: true`, `skipTaskbar: true`,
`focus: false`, `visible: false`. Ek stiller `WindowAdapter`'dan
`SetWindowLongPtr(GWL_EXSTYLE)` ile: `WS_EX_NOACTIVATE` (Tauri'nin `focus:false`'u tek
başına yetmiyor), `WS_EX_TOOLWINDOW`, `WS_EX_TRANSPARENT`.

Rust tarafı `docs/ARCHITECTURE.md`'deki katmanlara birebir oturur:

```
app/src-tauri/src/
  main.rs            pencereyi kurar, adapter thread'lerini baslatir, mpsc'yi baglar
  core/              PlayerState, gurultu kurallari, gorunurluk durum makinesi
  adapters/smtc.rs   SmtcAdapter
  adapters/audio.rs  AudioAdapter
  adapters/window.rs WindowAdapter
```

**Mimari kuralı burada uygulanır:** `unsafe`, WinRT tipi ve COM arayüzü yalnızca
`adapters/` içinde görünür. `core/` Windows'a dair hiçbir tip bilmez — test edilebilir
tek katman odur ve P2'nin testleri oraya yazılır.

### P2.1 — SmtcAdapter

`spikes/p1-smtc/src/main.rs`'deki abonelik kurulumu buraya taşınabilir (MTA
`CoInitializeEx`, oturum başına sök-tak, `CurrentSessionChanged`). **Taşınmayacak olan:**
spike'ın mutex'le tuttuğu durum. Adapter durum tutmaz; olayı domain tipine çevirip
mpsc'ye yazar — P1 § 7'de olayların eşzamanlı thread'lerde geldiği ölçüldü.

### P2.2 — AudioAdapter

`IMMDeviceEnumerator` → `IAudioEndpointVolume`, tetik `IAudioEndpointVolumeCallback::OnNotify`.
Cihaz değişimi `IMMNotificationClient` ile. Kendi thread'i, kendi apartment'ı.
**Bu aşamanın ölçülmemiş tek parçası** — beklenmedik davranışta tahmin etme, küçük bir
spike yaz.

### P2.3 — Gürültü kuralları

Dördü de yazılacak, ama **P1'in düzelttiği hâliyle** — gerekçeler `docs/P1-SONUC.md`
§ 6 ve § 8'de, oraya bakmadan yazma:

1. **İlk olayı yut.** Abonelikte olay gelmedi (§ 6.1), kural yine de koruma amaçlı kalır.
2. **Değişmeyeni yut.** Aynı (başlık, sanatçı, durum) üçlüsü tekrar gelirse at. Spotify
   aynı olayı 2 kez, Edge 3 kez gönderdi.
3. **Birleştirme penceresi animasyonu yönetir, gösterimi değil.** Tek parça geçişi
   811 ms'ye yayılan 8 olay üretiyor ve başlık 270. ms'de geliyor: ilk anlamlı olay
   kutuyu **hemen** açar, sonraki ~1 sn içindeki olaylar içeriği **yerinde** günceller
   ve deadline'ı ileri alır — animasyonu baştan oynatmaz.
4. **Ön plandakini atla** *(ayar, kapatılabilir)*.

Ayrıca P1'den gelen üç kesin kural:

- `CurrentSessionChanged` **tek başına tetik değildir** — yeni oturum boş doğuyor,
  içerik 14 ms sonra geliyor.
- `Opened` ve `Closed` durumları kutuyu açmaz. `Changing` tolere edilir ama beklenmez.
- `TimelinePropertiesChanged` ve `SessionsChanged` **tetik değildir** (duraklatılmışken
  bile geliyor).
- `LastUpdatedTime == 0` → pozisyon "bilinmiyor"; interpolasyona sokma.

### P2.4 — Görünürlük durum makinesi

`docs/ARCHITECTURE.md` → Görünürlük durum makinesi. Tek deadline, iki durum, tek
yazıcı görev. Gösterme/gizleme `ShowWindow` ile; **gizlerken gerçek `SW_HIDE`**,
opaklık sıfırlamak değil. Pencere bir kez oluşturulur, bir daha yok edilmez.

### Çıkış kriteri

1. Ses tuşuna 10 kez arka arkaya bas — **tek kutu**, üst üste binme yok, titreme yok.
2. Spotify penceresinden (fareyle) durdur — **kutu çıkıyor**; Windows'unki çıkmıyor.
   Projenin tek gerçek değeri budur, bu maddeyi atlama.
3. Parça değiştir — kutu bir kez açılıyor, başlık 270 ms sonra gelince **yerinde**
   güncelleniyor, animasyon iki kez oynamıyor.
4. Tarayıcı sekmesi ↔ Spotify geçişinde boş kutu **görünmüyor**.
5. `core/` katmanının gürültü kuralları birim testli ve `cargo test` geçiyor.

### P2'de yapılmayacaklar

- `design/` bağlanmayacak (P3), tema/DPI/monitör seçimi yok.
- Kapak görseli çözülmeyecek (P3).
- Windows'un kendi flyout'u bastırılmayacak (P4) — 2. maddedeki "Windows'unki
  çıkmıyor" ifadesi fare/uygulama kaynaklı durdurmada zaten çıkmadığı içindir.
- WebView2'nin gizliyken render'ı durdurulmayacak → **P3'e alındı** (`CLAUDE.md`'de
  "P2/P3" yazıyordu). Gerekçe: gerçek UI olmadan `IsVisible=false` / `TrySuspend`
  uygulanıp uyanma bedeli anlamlı biçimde yeniden ölçülemez.

</details>

---

## P3 — Arayüz ✅ TAMAMLANDI (2026-08-28)

**Tasarım bağlandı, sekiz durum uygulamada birebir çıkıyor, WebView2 gizliyken
render'ı duruyor.** Kararlar, ölçüm tablosu ve tuzaklar: **`docs/P3-SONUC.md`**.

- **P2'den devredilen iki senaryo doğrulandı:** ses tuşuna arka arkaya basma →
  tek kutu; Spotify penceresinden fareyle durdurma → kutu çıkıyor. Parça geçişi
  ve sekme ↔ Spotify geçişi de elle teyit edildi.
- **P0'ın açık bıraktığı ölçüm tekrarlandı:** `SetIsVisible(false)` ile gizliyken
  üretilen kare **9 235 → 0**, uyanma bedeli p95 **12.1 → 11.6 ms**. Uyanma
  cezası **çıkmadı**. `TrySuspend` bilerek kullanılmadı (JS'i de donduruyor,
  içerik-önce-pencere sırasını bozardı) — gerekçe `docs/P3-SONUC.md` § 5.
- **Hâlâ açık:** kalıcı tema/köşe/monitör ayarı **P5'e** (ayar dosyası); kulaklık
  takıp çıkarma ile cihaz değişimi doğrulaması yapılmadı.

<details>
<summary>Özgün P3 planı</summary>

`design/` altındaki CSS'i frontend'e bağla. Backend hazır: tek kanal (`flyout`),
tek payload (`core::engine::Frame`) — alan alan dökümü `docs/P2-SONUC.md` § 2'de,
**oraya bakmadan DOM yazma.**

**İlk iş (P2'den devir):** ses tuşuna arka arkaya basma ve Spotify penceresinden
fareyle durdurma senaryolarını mevcut yer tutucu kutuyla doğrula. Tasarım ondan
sonra bağlanır.

- `flyout.css` + iki tema dosyası. `app/src/styles.css` ve `index.html` **yer
  tutucu**; oradaki değerleri `design/`'a taşıma, tersi geçerli.
- `PlayerState` → DOM. `phase` animasyonu yönetiyor: `enter` giriş, `update`
  **animasyonsuz yerinde güncelleme** (başlık olaydan 270 ms sonra geliyor),
  `leave` çıkış. `position` **null gelebilir**, `album` boş, `controls.next`
  false olabilir.
- Kapak görselini gizliyken önden çöz ve cache'le
- Giriş/çıkış animasyonu, 150 ms
- Tema değiştirme (`data-theme` attribute), köşe ve monitör seçimi, DPI. Şu an
  pencere **birincil monitörün** sağ üst köşesine 16 px boşlukla konuluyor
  (`adapters/window.rs`).
- **AudioAdapter'ın cihaz değişimi yolu gerçek donanımla denenmedi** — kulaklık
  takıp çıkararak doğrula (`IMMNotificationClient` → yeniden bağlanma).
- **WebView2'nin gizliyken render'ını durdur** (`WebView::set_visible` / `TrySuspend`)
  ve **P0 ölçümünü tekrarla** — bugünkü 11 ms render döngüsü sıcakken alındı, asıl
  uyanma bedeli o zaman ortaya çıkacak (bkz. `docs/P0-SONUC.md` yan bulgu)

**Çıkış:** `design/preview.html`'deki sekiz durumun sekizi de uygulamada birebir aynı
görünüyor.

</details>

---

## P4 — Native flyout bastırma  ← **SIRADAKİ**

Ayrı ve en sonda, çünkü uygulama bu olmadan da kullanılabilir olmalı.

- `NativeFlyoutSuppressor` modülü, sürüm başına strateji
- Bulunamazsa sessiz vazgeç, ayarlarda durumu göster

**Çıkış:** Bastırma başarısız olduğunda uygulama çalışmaya devam ediyor.

---

## P5 — Paketleme

- Tepsi ikonu, Windows ile başlat
- Ayar dosyası (tema, köşe, süre, monitör)
- WebView2 bootstrapper gömülü installer

**Çıkış:** Temiz bir Windows kurulumunda çift tıkla çalışıyor.

---

## Sonraya (v2)

- Uygulama bazlı ses kontrolü (`ISimpleAudioVolume`)
- Kutu üzerinde etkileşim (`WS_EX_TRANSPARENT` kaldırılıp tıklanabilir mod)
- Diğer temalar: pixel/bitmap, lo-fi pixel, hi-fi kaset dekk
- Uzun şarkı adı için kaydırmalı (marquee) başlık — şu an kesiliyor
