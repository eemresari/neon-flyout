# Mimari

## Gecikme bütçesi — sistemi belirleyen tek gereksinim

Tetikleyiciden ilk piksele **50 ms**. Ses tuşuna basıp 200 ms sonra beliren bir kutu
bozuk hisseder. Bu bir optimizasyon hedefi değil, kabul kriteridir.

| Aşama | Bütçe | Nasıl tutulur |
|---|---:|---|
| Olay → Core | < 5 ms | Adapter thread'i doğrudan mpsc'ye yazar, ara katman yok |
| Core → WebView state | < 5 ms | Pencere ayakta, WebView yüklü; sadece bir event emit'i |
| DOM güncelleme | < 10 ms | Düzen hazır, sadece metin/görsel değişir; kapak önceden cache'li |
| `SW_SHOW` → ilk kare | < 30 ms | **Riskli adım.** Ölçülmeden bilinemez → P0 |

Sonuç: pencere uygulama açılışında bir kez oluşturulur ve bir daha yok edilmez.
Gösterme/gizleme sadece `ShowWindow`.

---

## Katmanlar

Kural: her katman yalnızca bir alttakini tanır.

### L5 — Presentation (WebView)
Sadece render. İş mantığı yok, zamanlayıcı yok, API çağrısı yok. Aldığı tek şey
`PlayerState` anlık görüntüsü. Uygulama açıldığı anda yüklenir, bir daha boşaltılmaz;
gizliyken de DOM ayakta durur.

### L4 — IPC sınırı
Yukarı doğru olay (`app.emit`), aşağı doğru komut (`tauri::command`).
Kutu görünürken pozisyon/spektrum 10 Hz akar; **gizliyken hiçbir şey gönderilmez** —
boşuna WebView uyandırma.

### L3 — Core
Tek doğruluk kaynağı `PlayerState` + tek bir görünürlük zamanlayıcısı. Tek yazıcı
görev, mpsc ile beslenir. Windows'a dair hiçbir tip bilmez — **test edilebilir olan
tek katman budur**.

### L2 — Adapters
Her biri kendi thread'inde, kendi COM apartment'ında. Dışarıya sadece domain tipleri
verir. WASAPI ve WinRT farklı thread modelleri ister; her adapter kendi
`CoInitializeEx`'ini yapar, handle'lar thread'ler arası paylaşılmaz.

- `SmtcAdapter` — medya oturumu
- `AudioAdapter` — ses seviyesi, mute, peak
- `WindowAdapter` — pencere stilleri, z-order, göster/gizle
- `NativeFlyoutSuppressor` — Windows'un kendi kutusunu gizler (izole, kırılgan, opsiyonel)

### L1 — Windows platformu
`Windows.Media.Control`, Core Audio (WASAPI), `user32` / `dwmapi`.
Bu katmanın üstünde hiçbir yerde `unsafe` blok olmamalı.

**Akış:**
`SMTC/ses olayı → Adapter thread → mpsc → Core + deadline → emit → WebView → SW_SHOW`

---

## Tetikleyiciler

Hiçbiri klavye dinlemeyi gerektirmiyor. Bu bir kolaylık değil, tasarımın merkezi.

| Tetikleyici | Kaynak | Not |
|---|---|---|
| Parça değişti | `MediaPropertiesChanged` | Aynı parça için tekrar gelirse yut |
| Durdur / başlat | `PlaybackInfoChanged` | Kaynağı ayırt etmeye gerek yok |
| Ses değişti | `IAudioEndpointVolumeCallback::OnNotify` | Donanım tuşu, mixer, kulaklık — hepsi |
| Kaynak değişti | `CurrentSessionChanged` | Eski oturumun aboneliklerini sökmeyi unutma |
| Elle çağırma | `RegisterHotKey` *(opsiyonel)* | Çekirdek davranış için gerekli değil |

### Durum dinlemenin bedeli: gürültü

Bu dört kural olmadan kutu kendi kendine açılıp durur ve Windows'unkinden beter olur.
**Dördü de P2'de yazılmalı**, sonraya bırakılırsa alışkanlık haline gelir.

1. **İlk olayı yut.** Abone olunduğu anda mevcut durum bir olay olarak gelir; yoksa
   uygulama her açılışta kendini gösterir.
2. **Değişmeyeni yut.** Aynı `PlaybackStatus` + aynı parça için tekrar gelen olayı at.
3. **~150 ms birleştir.** Parça geçişi tek olay değil, `Paused → Changing → Playing`
   zinciri gelir; birleştirilmezse kutu titrer.
4. **Ön plandakini atla** *(ayar, kapatılabilir)*. Kaynak uygulamanın PID'si ön plandaki
   pencereyle aynıysa kutuyu gösterme.

---

## Görünürlük durum makinesi

İki durum ve bir zaman damgası. Karmaşıklığı buraya sokma; flyout'ların bozuk
hissetmesinin sebebi genelde üst üste binen zamanlayıcılardır.

```
Hidden ──(dört tetikten biri)──► Visible   deadline = now + 2500 ms
Visible ──(yeni tetik)─────────► Visible   deadline'ı ileri al, içeriği güncelle
                                            YENİ PENCERE AÇMA, ANİMASYONU BAŞTAN OYNATMA
Visible ──(deadline doldu)─────► FadeOut   150 ms
FadeOut ──(fade bitti)─────────► Hidden    SW_HIDE  ← opaklık sıfırlamak DEĞİL
```

---

## Adapter'lar ve API'ler

| Modül | API | Dikkat |
|---|---|---|
| smtc / oturum | `GlobalSystemMediaTransportControlsSessionManager::RequestAsync` | `CurrentSessionChanged` ile aktif oturumu izle; oturum başına abonelikleri sök-tak |
| smtc / pozisyon | `GetTimelineProperties()` | SMTC sürekli pozisyon yayınlamaz, olaylar seyrek. `Instant` ile interpolate et, olay geldiğinde resync |
| smtc / yetenek | `GetPlaybackInfo().Controls` | `IsNextEnabled`, `IsPlaybackPositionEnabled` … Her kaynak seek desteklemez — butonu pasifleştir |
| smtc / kapak | `TryGetMediaPropertiesAsync().Thumbnail` | `IRandomAccessStreamReference` → byte. **Gizliyken önden çöz ve cache'le** — 50 ms içinde decode edilemez. (başlık+sanatçı) hash'iyle sakla |
| audio / sistem | `IMMDeviceEnumerator` → `IAudioEndpointVolume` | Seviye + mute + `IAudioEndpointVolumeCallback` ile tetikleme. Cihaz değişimini `IMMNotificationClient` ile yakala |
| audio / uygulama | `IAudioSessionManager2` → `ISimpleAudioVolume` | Uygulama bazlı ses. Çekirdek için gerekli değil, v2'ye bırakılabilir |
| audio / seviye | `IAudioMeterInformation::GetPeakValue` | Spektrum/VU için doğru araç, ses yakalamaya gerek yok. **Sadece kutu görünürken oku** |
| window / stil | `SetWindowLongPtr(GWL_EXSTYLE)` | `WS_EX_NOACTIVATE` (Tauri'nin `focus:false`'u tek başına yetmiyor), `WS_EX_TOOLWINDOW`, `WS_EX_TRANSPARENT` |
| window / z-order | `SetWindowPos(HWND_TOPMOST, …)` | Her göstermede `SWP_NOACTIVATE` ile. Gizliyken hiçbir şey yapma |
| suppressor | `FindWindow` / `ShowWindow` | Belgelenmemiş. Sürüm başına strateji, bulunamazsa **sessiz vazgeç** |

---

## Windows'un kendi flyout'unu bastırma

Resmî bir ayar veya kayıt defteri anahtarı **yok**. Mevcut uygulamalar (ModernFlyouts,
HideVolumeOSD, FluentFlyout) native pencereyi sınıf adıyla bulup gizliyor —
belgelenmemiş bir yöntem. Windows 11 22H2 bu pencereyi değiştirdi ve ModernFlyouts'un
yöntemi kırıldı; 24H2 için ayrı bir fork açıldı. **Bu kod her büyük Windows
güncellemesinde bozulacak.**

Mimari karşılık: `NativeFlyoutSuppressor` tek bir değiştirilebilir modül, sürüm başına
strateji tutar, hedef pencere bulunamazsa çökmez — sessizce vazgeçer ve ayarlarda
"sistem flyout'u gizlenemedi" durumunu gösterir. **Uygulamanın geri kalanı bu modül
olmadan da çalışmalı.**

Öncelik düşük: kullanıcının üç tetikleme yolundan yalnızca klavyedeki medya tuşu
Windows'un kutusunu da açıyor. Bu yüzden P4'te, en sonda.

Kod okunacak yer:
- <https://github.com/unchihugo/FluentFlyout>
- <https://github.com/ModernFlyouts-Community/ModernFlyouts>

---

## Riskler

| Risk | Seviye | Karşılık |
|---|---|---|
| WebView2 uyanma gecikmesi | Yüksek | Projenin tek go/no-go riski. P0'da ölçülür, WPF fallback'i önceden ilan edilmiş |
| Native flyout gizleme kırılır | Yüksek | İzole modül + sessiz vazgeçme |
| Eksik SMTC metadata | Orta | Bazı tarayıcı sekmeleri ve oyun içi sesler SMTC'ye kaydolmaz. Boş durum için zarif fallback, hata gibi gösterme |
| Olay gürültüsü | Orta | Yukarıdaki dört kural, P2'de |
| Focus çalma | Orta | `WS_EX_NOACTIVATE` + her `SetWindowPos`'ta `SWP_NOACTIVATE` |
| COM apartment hataları | Orta | Her adapter kendi thread'i, kendi `CoInitializeEx`'i |
| Direct Flip sekmesi | Düşük | 2.5 sn sürdüğü için kabul edilebilir — gerçekten `SW_HIDE` yapıldığı sürece |
| Anticheat | Düşük | Injection yok, DirectX hook yok, klavye hook yok. Sadece normal bir pencere |
| Exclusive fullscreen | Kapsam dışı | Kompozitör devre dışı, hiçbir normal pencere görünemez |
