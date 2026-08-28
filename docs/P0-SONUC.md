# P0 — gecikme kapısı sonucu

**Tarih:** 2026-08-28 · **Karar: GEÇTİ → yığın `Tauri v2 + Rust` olarak kalır.**

En kötü Tauri koşusu **12.9 ms** (p95), bütçe **50 ms**. Kapı geniş farkla geçildi ve
WPF ile arada ölçülebilir bir üstünlük yok — dolayısıyla `design/` altındaki üretim
CSS'ini XAML'e yeniden yazmayı gerektirecek hiçbir gerekçe kalmadı.

---

## Ölçüm makinesi

| | |
|---|---|
| CPU / RAM | AMD Ryzen 5 7600X (6c) · 31.2 GB |
| GPU / ekran | NVIDIA GeForce RTX 5070 · 2560×1440 @ **184 Hz** (bir kare = 5.43 ms) |
| OS | Windows 11 Pro build 26200 |
| WebView2 Runtime | 151.0.4129.107 |
| Rust / Tauri | rustc 1.98.0 (MSVC) · tauri 2.11.5 |
| .NET | SDK 8.0.424, WindowsDesktop 8.0.29 |

Her iki spike da **release** derlemesiyle, aynı profillerle, aynı `ShowWindow` yoluyla
ve aynı ölçüm tekniğiyle koştu. Isınma olarak ilk 3 tekrar atıldı (CSV'ye yazılmadı).

---

## Yöntem — ve ilk yöntemin neden çürütüldüğü

ROADMAP "ilk kareyi `requestAnimationFrame` ile yakala" diyordu. **Bu yöntem denendi ve
geçersiz çıktı.** Sağlık sayacı (`idle_ticks`: gizli bekleme boyunca ateşlenen kare
sayısı) şunu gösterdi:

| Düzenek | Gizliyken üretilen kare | Sonuç |
|---|---|---|
| WebView2 + ham `SW_HIDE` | 370 / 2 sn = **185/sn** (tam tazeleme hızı) | rAF hiç durmuyor |
| WebView2 + Tauri `window.hide()` | 370 / 2 sn | yine durmuyor |
| WPF + `CompositionTarget.Rendering` | ~127 / 2 sn (~60/sn) | MediaContext tikliyor |

`visible_while_idle=0`, yani Win32 pencereyi gerçekten gizli sayıyor — ama render
döngüleri buna aldırmıyor. Dolayısıyla "gösterimden sonraki ilk callback" bir *uyanma*
ölçmüyor, sadece "zaten dönen döngüde bir sonraki kare"yi ölçüyor. Bu sayılarla kapı
kararı verilseydi karar yanlış temele oturmuş olurdu.

**Kullanılan yöntem — `screen_ms`:** masaüstü DC'sinden tek bir piksel sıkı döngüde
yoklanır; `t0` (gösterme çağrısından hemen önceki `QueryPerformanceCounter`) ile rengin
değiştiği an arasındaki fark ölçülür. Hiçbir çerçeve callback'ine güvenmez, **DWM'in
gerçekten sunduğu kareyi** görür ve iki yığında birebir aynıdır. Yoklama noktası her
koşuda `GetWindowRect`'ten hesaplanır ve pencere/masaüstü renklerinin ayırt edilebildiği
doğrulanır (koşu başında `GECERLI` satırı basılır).

Sapmalar, dürüstlük payı olarak: `GetPixel` çağrısının kendi maliyeti (~0.1 ms) ölçüme
dahildir ve muhafazakâr yöndedir; yoklama döngüsü bir çekirdeği doldurur. `raf1`/`raf2`
sütunları CSV'de duruyor ama **karar için kullanılmadı**, sadece yukarıdaki teşhis için.

---

## Sonuçlar — `screen_ms` (tetik → ekrandaki piksel)

| Koşu | n | p50 | **p95** | max |
|---|---:|---:|---:|---:|
| `tauri-hide-short` | 25 | 11.1 | **12.1** | 12.4 |
| `tauri-hide-long` (60 sn bekleme) | 5 | 11.3 | **12.0** | 12.0 |
| `tauri-hide-short-tauriapi` | 25 | 11.5 | **12.9** | 13.2 |
| `tauri-offscreen-short` | 25 | 10.9 | **11.8** | 11.9 |
| `wpf-hide-short` | 25 | 10.9 | **11.1** | 11.2 |
| `wpf-hide-long` (60 sn bekleme) | 5 | 10.8 | **11.1** | 11.2 |
| `wpf-offscreen-short` | 25 | 10.5 | **10.7** | 10.8 |

Sayılar ms. Ham veri `measurements/*.csv` (git'e girmiyor), özet için
`python spikes/analyze.py`.

### Okunacak dört şey

1. **Kapı geçildi, hem de rahat.** ~11–13 ms, 50 ms bütçesinin dörtte biri. Mimarideki
   "`SW_SHOW` → ilk kare < 30 ms" satırı fazlasıyla karşılanıyor.
2. **Tauri ile WPF arasında pratik fark yok.** ~1–2 ms, iki kare arası mesafenin bile
   altında. Yığın kararını gecikme belirlemiyor — bu yüzden karar, tasarımın hazır
   olduğu tarafa düşüyor.
3. **Derin bekleme bedava.** 60 saniye gizli kalmak 2 saniye gizli kalmakla aynı
   (11.3 vs 11.1). Yol haritasında olmayan ama gerçek kullanımı temsil eden profil
   buydu; korkulan "uzun süre gizli kalınca uyanma pahalılaşır" senaryosu **çıkmadı**.
4. **`offscreen` alternatifi ölü.** Ekran dışına taşımak 11.8 ms veriyor — `SW_HIDE`'ın
   12.1 ms'inden anlamlı biçimde iyi değil. Karşılığında pencere kompozisyonda kalıyor
   (`visible_while_idle=1`) ve Direct Flip bedeli sürekli ödeniyor. **`SW_HIDE` kararı
   doğrulandı, alternatif kapandı.**

Not: 11 ms ≈ 184 Hz'de tam iki kare. Ölçüm, tazeleme hızına oturmuş temiz bir sayı —
gürültü değil.

---

## İkinci soru: `backdrop-filter` şeffaf pencerede çalışıyor mu?

**Hayır — şeffaf bölgede hiçbir şey yapmıyor.**

Test: pencerenin sağ 110 px'i bilerek şeffaf bırakıldı, oraya `backdrop-filter` uygulanan
bir kutu ve hemen altına aynı boyutta **filtresiz kontrol kutusu** kondu. "Renk biraz
değişti mi?" göz kararına düşmemek için filtre `grayscale(1) blur(10px)` yapıldı —
çalışsaydı o bölgede R=G=B olmak zorundaydı.

| Kanıt | Sonuç |
|---|---|
| `grayscale(1)` bölgesinde renk saçılımı | **310** (gri olsaydı ~0) |
| Pencere var/yok arasında bölge farkı | 14 (arka planın kendi değişimi kadar) |
| Ekran görüntüsü | Kutunun kenarlığı ve metni görünüyor, **arkası değişmemiş** |

Yani eleman boyanıyor; filtre uygulanmıyor. Sebep mimari: CSS'te "backdrop" sayfanın
kendi kompozisyon zemini demek, şeffaf bir pencerenin arkasındaki **işletim sistemi
masaüstü buna dahil değil** — WebView2 onu örnekleyemiyor.

**Etkisi yok.** `docs/DESIGN.md` zaten kontrastı opak plakaya yükledi ve tasarımların
hiçbiri blur'a bağlı değil. Kayıt için: sayfanın *kendi* içeriği üzerinde blur denenmedi;
P3'te bir yerde gerekirse ayrıca sınanmalı.

---

## Yan bulgu — mimariye yazılması gereken

**`SW_HIDE` WebView2'nin render döngüsünü durdurmuyor.** Pencere Win32'ye göre gizli
(`visible_while_idle=0`) olmasına rağmen WebView saniyede 185 kare üretmeye devam ediyor;
60 saniyelik koşuda 11 098 kare. Tauri'nin kendi `window.hide()` yolu da bunu değiştirmedi.

Gecikme açısından bu **iyi haber** — kapının rahat geçilmesinin sebebi de bu, ortada
gerçek bir "uyanma" yok. Ama `docs/ARCHITECTURE.md`'nin Direct Flip gerekçesi güç
tüketimini de ima ediyordu: pencere gizliyken kompozisyona girmiyor, dolayısıyla Direct
Flip korunuyor, **fakat GPU boşuna çalışıyor.**

Kutu günün büyük kısmında gizli duracağı için bu, dizüstü pil ömrü açısından ciddiye
alınmalı. P2/P3'te çözülmesi gereken açık iş: gizlenirken WebView2 controller'ının
`IsVisible`'ını `false`'a çekmek (wry'de `WebView::set_visible`) ya da `TrySuspend`
kullanmak — **ve sonra bu ölçümü tekrarlamak**, çünkü asıl uyanma bedeli o zaman ortaya
çıkacak. Bugünkü 11 ms, render döngüsü sıcakken alınmış bir sayıdır.

---

## Tekrarlamak için

```bash
cd spikes/p0-tauri && npm install && npm run tauri build --no-bundle
# profil: hide-short | hide-long | offscreen-short ; gizleme: raw | tauri
P0_PROFILE=hide-short P0_HIDE=raw ./src-tauri/target/release/p0-tauri.exe

cd ../p0-wpf && dotnet build -c Release
# gizleme: raw | wpf
P0_PROFILE=hide-short P0_HIDE=raw ./bin/Release/net8.0-windows/p0-wpf.exe

python spikes/analyze.py
```

### Ortam engeli — bilinmesi gereken

Bu makinede **Smart App Control açık** (`VerifiedAndReputablePolicyState = 1`) ve
imzasız Rust ikilisini hash başına, öngörülemez biçimde engelliyor: aynı kaynaktan
üretilen derlemelerin bir kısmı çalıştı, bir kısmı `Uygulama Denetimi ilkesi bu dosyayı
engelledi` hatası verdi. .NET tarafı etkilenmedi (apphost Microsoft'un bilinen şablonu
olduğu için itibar denetiminden geçiyor).

P1'den itibaren sürekli Rust derlemesi yapılacağı için bu tekrar tekrar karşıya çıkacak.
Kalıcı çözüm ikisinden biri: Smart App Control'ü kapatmak (**geri alınamaz** — tekrar
açmak Windows'u sıfırlamayı gerektirir) veya derlemeleri güvenilen bir sertifikayla
imzalamak. Karar verilmedi.
