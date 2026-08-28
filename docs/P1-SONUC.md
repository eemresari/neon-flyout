# P1 — SMTC spike sonucu

**Tarih:** 2026-08-28 · **Karar: GEÇTİ.** SMTC paketlenmemiş exe'den sorunsuz okunuyor,
oturum değişimi yakalanıyor, komut gönderilebiliyor, metadata doğru akıyor.

Düzenek: `spikes/p1-smtc` (Rust konsol uygulaması, `windows` 0.61, MTA apartment).
Çıkış kriteri **Spotify ↔ tarayıcı sekmesi geçişinde doğru metadata** iki ayrı
tarayıcıyla (Opera GX ve Edge) sağlandı.

Aşağıdaki bloklar koşu kayıtlarından birebir alıntı. Kayıtlar makine yereli.

---

## 1. Capability sorunu **yok**

`GlobalSystemMediaTransportControlsSessionManager::RequestAsync()` paketlenmemiş,
imzasız bir Win32 exe'den **hiçbir bildirim olmadan** çalıştı.

```
[   0.001s] SMTC yoneticisi isteniyor (RequestAsync)...
[   0.011s]   ALINDI (9.8 ms) — paketlenmemis exe, capability bildirimi yok
```

Manifest yok, `globalMediaControl` yok, UAC yok. İlk (soğuk) çağrı 30 ms, ısındıktan
sonra ~10 ms sürüyor — uygulama açılışında bir kez ödenir, gecikme bütçesinin
dışındadır. **`CLAUDE.md`'deki 2. varsayım kapandı.**

> Yalnızca paketlenmemiş dağıtım için geçerli. P5'te MSIX paketlenirse capability
> bildirimi tekrar gündeme gelir.

## 2. Smart App Control engeli bu turda çıkmadı

P0'da Rust ikililerini rastgele engelleyen SAC, bu spike'ın hiçbir derlemesini
engellemedi (dört ayrı release derlemesi, dördü de çalıştı). Sorun **ortadan
kalkmadı**, sadece tetiklenmedi — karar hâlâ açık.

---

## 3. Metadata doğru ve hızlı

Spotify oturumu:

```
kaynak    : SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify
durum     : Paused
pozisyon  : 2:12 / 3:32 (yas 2663 ms)
yetenekler: play toggle next prev stop seek shuffle repeat
baslik: "Telephones"   sanatci: "Vacations"   album: "Changes"   kapak: var
```

`TryGetMediaPropertiesAsync().get()` **1 ms'den kısa sürüyor**. Metadata çekimi
gecikme bütçesi için sorun değil; pahalı olan kapak *decode*'u — o hâlâ P3'ün işi.

### Tarayıcı oturumları farklı şekilde geliyor

Gerçek bir Opera GX sekmesi (video):

```
kaynak    : OperaSoftware.OperaGXWebBrowser.1783353490
yetenekler: pause toggle stop seek        ← next/prev YOK
baslik    : "Kötüye dönüştükten sonra daha güçlü olan 3 Marvel karakteri!! ..."
sanatci   : "Bunu Zaten İzledim"          ← sanatçı değil, kanal adı
album     : ""                            ← boş
```

Üç sonuç:
- **`album` boş gelebilir** — tasarım bunu boşluk bırakarak değil, satırı toplayarak
  karşılamalı (P3).
- **Başlık çok uzun olabilir.** `design/` kesiyor; v2'deki marquee fikri buradan
  besleniyor.
- **`yetenekler` kaynağa göre değişiyor.** `IsNextEnabled=false` olan kaynakta buton
  pasifleştirilmeli — ARCHITECTURE'ın notu doğrulandı.

## 4. Komut gönderme çalışıyor

```
[   2.005s] KOMUT TryTogglePlayPauseAsync   sonuc=true
[   2.038s] OLAY  TimelinePropertiesChanged pozisyon=2:12
[   2.040s] OLAY  PlaybackInfoChanged      durum=Playing
```

Çağrı ~1 ms'de dönüyor, karşılığındaki durum olayı ~35 ms sonra geliyor.
`TrySkipNextAsync` da çalıştı. `TrySkipPreviousAsync` Spotify'da **parçayı başa
sarıyor**, önceki parçaya dönmüyor (üst üste iki kez basıldığında bile) — kaynak
uygulamanın davranışı, API'nin değil.

---

## 5. Oturum değişimi — çıkış kriteri

İki yönde de doğrulandı.

**Tarayıcı → Spotify** (Opera GX sekmesindeki video bitti):

```
[   5.204s] OLAY  PlaybackInfoChanged      durum=Closed     ← ölmekte olan oturumdan
[   5.204s] OLAY  CurrentSessionChanged
[   5.204s] OLAY  SessionsChanged            oturum sayisi=1
[   5.204s]   eski oturumun abonelikleri sokuldu
[   5.204s]   yeni oturuma abone olundu
[   5.204s]   kaynak: ...!Spotify   baslik: "Anhedoni"   sanatci: "VAGON"
```

**Spotify → tarayıcı** (Edge'de test sayfası çalmaya başladı):

```
[  17.407s] OLAY  CurrentSessionChanged
[  17.407s]   kaynak: MSEdge  durum: Opened  baslik: ""  sanatci: ""  kapak: yok
[  17.421s] OLAY  MediaPropertiesChanged
[  17.421s]   baslik: "Test Parcasi A"  sanatci: "Tarayici Oturumu"
[  17.422s] OLAY  PlaybackInfoChanged      durum=Playing
```

### 5.1 ⚠ Yeni oturum **boş** doğuyor — P2/P3'ü doğrudan ilgilendirir

`CurrentSessionChanged` geldiğinde oturumun metadata'sı **henüz yok**: başlık boş,
durum `Opened`, süre 0:00, kapak yok. Gerçek içerik **14 ms sonra**
`MediaPropertiesChanged` ile geliyor.

**Kural:** kutuyu `CurrentSessionChanged` üzerine açma. Boş bir kutu görünür.
Tetik, içeriği dolu ilk olaydır (`MediaPropertiesChanged` veya `PlaybackStatus`
gerçekten `Playing`/`Paused` olan bir `PlaybackInfoChanged`). `Opened` ve `Closed`
durumları **gösterim tetikleyicisi değildir**.

### 5.2 `LastUpdatedTime` sıfır gelebilir

Yeni doğan oturumda:

```
pozisyon  : 0:00 / 0:00 (yas 13432397154926 ms)
```

Yaş ≈ 425 yıl, çünkü `LastUpdatedTime` sıfır `DateTime` (1601). İnterpolasyon kodu
bunu bölmeye/çıkarmaya sokarsa saçma pozisyon üretir. **`UniversalTime == 0` ise
pozisyon "bilinmiyor" sayılmalı.**

---

## 6. Olay akışının gerçek şekli — P2'nin girdisi

Spike'ın asıl kazancı. `docs/ARCHITECTURE.md`'deki dört gürültü kuralının üçü
doğrulandı, biri düzeltilmeli.

### 6.1 "İlk olayı yut" — bu makinede tetiklenmedi, kural yine de kalsın

Abone olunduğu anda hiçbir olay gelmedi; ilk olay 1.8 saniye sonra ilgisiz bir
`TimelinePropertiesChanged`'di. WinRT abonelikte mevcut durumu tekrar yayınlamıyor.
Kural koruma amaçlı yazılacak, ama **asıl tehlike bu değil, 5.1'deki boş oturum.**

### 6.2 "Değişmeyeni yut" — **gerekli, iki kez kanıtlandı**

Spotify'da tek bir parça geçişi **iki adet birebir aynı** `MediaPropertiesChanged`
üretti (1 ms arayla). Edge'de aynı `PlaybackInfoChanged` **üç kez** arka arkaya geldi:

```
[  17.422s] OLAY  PlaybackInfoChanged      durum=Playing
[  17.422s] OLAY  PlaybackInfoChanged      durum=Playing
[  17.422s] OLAY  PlaybackInfoChanged      durum=Playing
```

Ve tarayıcı, çalarken kendiliğinden durum çırpındı:

```
[  23.269s] Paused   [  23.874s] Playing   [  24.241s] Paused
```

Bu kural olmadan kutu saniyede birkaç kez yeniden tetiklenir.

### 6.3 "~150 ms birleştir" — gerekli, ama pencere yetersiz

Tek bir "sonraki parça" komutunun ürettiği tam seri:

| t (komuttan itibaren) | Olay |
|---:|---|
| +67 ms | TimelinePropertiesChanged (0:00) |
| +67 ms | PlaybackInfoChanged (Playing) |
| +213 ms | TimelinePropertiesChanged |
| +270 ms | MediaPropertiesChanged |
| +271 ms | TimelinePropertiesChanged |
| +271 ms | MediaPropertiesChanged *(aynısı)* |
| +340 ms | TimelinePropertiesChanged |
| +811 ms | TimelinePropertiesChanged |

**Tek kullanıcı eylemi 8 olay** üretti, seri 811 ms'ye yayıldı. 150 ms'lik pencere
bunun sadece ilk ikisini toplar; **başlık 270. ms'de geliyor.**

**P2 için düzeltilmiş kural:** birleştirme penceresi *kutuyu açma* kararını değil,
*animasyonu yeniden başlatma* kararını kontrol eder. İlk anlamlı olay kutuyu hemen
açar (gecikme bütçesi bunu gerektiriyor); sonraki ~1 sn içindeki olaylar **görünen
kutunun içeriğini yerinde günceller**, deadline'ı ileri alır, animasyonu baştan
oynatmaz. "Tek olay = tek gösterim" beklentisi yanlış.

### 6.4 `Changing` durumu görülmedi

ARCHITECTURE'ın andığı `Paused → Changing → Playing` zinciri Spotify'da çıkmadı; tek
bir `PlaybackInfoChanged` ile doğrudan `Playing`'e geçildi. Zincire *güvenilemez*,
ama `Changing` ve `Opened`/`Closed` yine de tolere edilmeli.

### 6.5 `TimelinePropertiesChanged` tetikleyici **değildir**

20 saniyelik koşuda 13 timeline olayı geldi; bir kısmı **hiçbir şey olmazken, çalma
duraklatılmışken** (t=1.853s, t=6.360s). Tetikleyici listesine girerse kutu
kendiliğinden açılıp durur. ARCHITECTURE'ın tablosu doğru; bu, nedeninin kanıtı.

Sayaçlar (20 sn, bir toggle çifti + bir parça geçişi):
`props=2 playback=3 timeline=13 currentSession=0 sessions=0`

### 6.6 Pozisyon bayat geliyor — interpolasyon şart

`LastUpdatedTime` yaşı: sakin anlarda **2.5–3.7 saniye**, olay anında 3–60 ms. SMTC
pozisyonu sürekli yayınlamıyor. ARCHITECTURE'ın "`Instant` ile interpolate et, olay
geldiğinde resync" kararı doğrulandı (sıfır `LastUpdatedTime` istisnasıyla, bkz. 5.2).

---

## 7. Threading — adapter tasarımını doğruluyor

MTA'da olaylar **thread pool thread'lerinde ve eşzamanlı** geliyor. Kayıtta
`CurrentSessionChanged` işlenirken araya `SessionsChanged` girdi:

```
[  17.407s] OLAY  CurrentSessionChanged
[  17.407s]   eski oturumun abonelikleri sokuldu
[  17.407s] OLAY  SessionsChanged            oturum sayisi=2     ← başka thread
[  17.407s]   yeni oturuma abone olundu
```

Spike'ı ayakta tutan tek şey abonelik durumunu koruyan mutex'ti. Gerçek uygulamada
karşılığı ARCHITECTURE'ın kararı: **adapter olayı domain tipine çevirir ve mpsc'ye
yazar; durum tek yazıcı görevde tutulur.** Adapter'ın içinde paylaşılan durum
tutulmamalı.

Ayrıca `PlaybackInfoChanged(Closed)` olayı **ölmekte olan oturumdan** geldi —
handler artık geçerli olmayan bir oturumu okuyabilir. Hata değil, sessizce atılmalı.

---

## 8. P2'ye devredilen kesin maddeler

1. Tetik kümesi: `MediaPropertiesChanged` + `PlaybackInfoChanged` (+ ses olayı).
   `TimelinePropertiesChanged` ve `SessionsChanged` **tetik değil**.
2. `Opened` / `Closed` durumları kutuyu açmaz.
3. `CurrentSessionChanged` tek başına kutuyu açmaz — içerik 14 ms sonra geliyor.
4. Aynı (başlık, sanatçı, durum) üçlüsü için gelen tekrarlı olay yutulur.
5. Birleştirme penceresi animasyonu değil, içerik güncellemesini yönetir (6.3).
6. `LastUpdatedTime == 0` → pozisyon bilinmiyor.
7. Adapter durum tutmaz; olaylar mpsc ile tek yazıcıya gider.

## 9. Spike'ın kendi bulduğu tuzak

Tarayıcı oturumu üretmek için yazılan test sayfası (`spikes/p1-smtc/browser-session.html`)
ilk denemede SMTC'de **hiç görünmedi**: Chromium yalnızca **≥ 5 saniye** süren medyayı
global medya denetimlerine veriyor. 2 saniyelik döngü görünmezken 30 saniyelik döngü
anında göründü. Tarayıcı tarafını test ederken bilinmesi gereken bir eşik.
