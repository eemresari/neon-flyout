# Overlay Flyout — proje brifingi

Windows için masaüstü uygulaması. Sistem medya oturumunu okur; parça değiştiğinde,
çalma durumu değiştiğinde veya ses ayarlandığında ekranın sağ üst köşesinde ~2.5
saniyelik bir kutu gösterip kaybolur. Windows'un kendi medya/ses flyout'unun yerine
geçer. Kendi çalma motoru **yok**.

Hedef makine: Windows 11 x64. Minimum: Windows 10 1809 (build 17763).

---

## İlk oturumda sırayla yapılacaklar

1. Bu dosyayı ve `docs/` altındaki dosyaları oku — **`P0-SONUC.md` ve `P1-SONUC.md`
   dahil.**
2. **P0 tamamlandı (2026-08-28): kapı geçildi, yığın Tauri v2 + Rust.** Tekrar ölçme.
3. **P1 tamamlandı (2026-08-28): SMTC okunuyor, oturum geçişi doğrulandı.** Tekrar
   ölçme; olay akışının gerçek şekli `docs/P1-SONUC.md`'de.
4. **`docs/ROADMAP.md` → P2'den (tetik + zamanlayıcı) başla**, sonra sırayla devam.
   P2 bölümü P1 ölçümüne göre yeniden yazıldı ve adım adım (P2.0–P2.4) veriliyor;
   ilk adım uygulama iskeletini kurmak — **henüz yok**, `app/` altında olacak.

`spikes/` altındaki üç proje P0 ve P1'in ölçüm düzeneğidir — **atılacak koddur**,
uygulamanın iskeleti değil. Uygulama ayrıca kurulacak.

Tasarım hazır ve `design/` altında gerçek CSS olarak duruyor — yeniden tasarlama,
oradan al.

---

## Neden bu uygulama var

Windows'un flyout'u **medya tuşuna basılmasına** tepki veriyor, **çalma durumunun
değişmesine** değil. Kullanıcı çoğu zaman fare makrosundan, Spotify penceresinin
kendisinden veya kulaklıktan durduruyor — o durumlarda ortada basılmış bir tuş yok,
dolayısıyla kutu da çıkmıyor.

Bu uygulama `PlaybackInfoChanged` olayını dinler. Komutun nereden geldiği önemli
değildir. **Projenin tek gerçek değeri bu.** Daha güzel bir kutu ikincil.

---

## Değişmez kararlar

| Karar | Detay |
|---|---|
| Veri kaynağı | Windows SMTC (`Windows.Media.Control`). Kendi player yok. |
| Tetik | Durum değişimi olayları. Klavye hook'u **yok**. |
| Görünürlük | Varsayılan gizli. Gösterirken `SW_SHOW`, gizlerken gerçek `SW_HIDE`. |
| Oyun desteği | Borderless windowed. Exclusive fullscreen **kapsam dışı** — injection yok. |
| Temalar | `cyber` ve `neon`. İkisi de tamamlanmış durumda. |
| Boyut | Medya kutusu 400×120 px, ses kutusu 400×84 px. |

### Neden `SW_HIDE`, alpha 0 değil

Oyunun üstünde görünen her piksel DWM'in Direct Flip yolunu kapatır ve o süre boyunca
G-Sync/FreeSync devre dışı kalır. Kutu 2.5 saniye durduğu için bu kalıcı bir kayıp
değil, kısa bir sekme. Ama pencere açık kaldığı sürece bedel devam eder — bu yüzden
opaklığı sıfırlayıp bırakmak yanlış, pencereyi gerçekten gizlemek gerekiyor.

---

## Yığın

**Tauri v2 + Rust**, native erişim için [`windows`](https://crates.io/crates/windows)
crate'i (SMTC, WASAPI ve Win32'nin tamamı Rust'tan çağrılabiliyor, ek native addon
gerekmiyor). Arayüz WebView2'de HTML/CSS.

**Karar verildi (2026-08-28, P0):** Tauri p95 **12.9 ms**, bütçe 50 ms → kapı geçildi.
WPF referansı 11.1 ms; aradaki ~1–2 ms 50 ms bütçesi karşısında anlamsız, dolayısıyla
tasarımı XAML'e taşımayı haklı çıkaracak bir gerekçe yok. **WPF yedek planı kapandı.**
Ayrıntı: `docs/P0-SONUC.md`.

Proje iskeleti şu komutla kurulur — elle uydurma:

```bash
npm create tauri-app@latest
```

---

## Doğrulanmamış varsayımlar — tahmin etme, ölç

Bunlar bilerek açık bırakıldı. Herhangi birini "muhtemelen şöyledir" diye kabul edip
üstüne kod yazma; küçük bir spike yaz, ölç, sonucu buraya yaz.

1. **Windows'un native flyout'unu gizlemenin güncel yöntemi.** Belgelenmemiş; sürümden
   sürüme değişiyor (`XamlExplorerHostIslandWindow`, `NativeHWNDHost` isimleri geçiyor
   ama hangisinin doğru olduğu doğrulanmadı). Kaynak: FluentFlyout ve ModernFlyouts
   depolarındaki güncel handler kodu.
2. **WebView2 gizliyken render'ı durdurulunca uyanma bedeli ne olur?** P0'da ölçülen
   11 ms, render döngüsü **sıcakken** alındı — `SW_HIDE` WebView2'yi durdurmuyor (bkz.
   aşağıdaki bulgu). `IsVisible=false` / `TrySuspend` uygulandığında gerçek uyanma bedeli
   ortaya çıkacak ve **ölçüm tekrarlanmalı.**

## P0'da cevaplanan varsayımlar

- ~~Gizli WebView2 penceresi `SW_SHOW`'dan sonra kaç ms'de ilk kareyi basar?~~
  **11.1 ms (p50) / 12.9 ms (p95)**, bütçe 50 ms. 60 sn derin bekleme ek maliyet
  getirmiyor. Ekran dışına taşıma alternatifi kazanç sağlamıyor → `SW_HIDE` doğrulandı.
- ~~Şeffaf Tauri penceresinde `backdrop-filter` çalışıyor mu?~~ **Hayır**, şeffaf bölgede
  hiçbir şey yapmıyor (CSS "backdrop"ı sayfanın kendi zemini; masaüstü buna dahil değil).
  Tasarımlar blur'a bağımlı olmadığı için etkisi yok.

## P1'de cevaplanan varsayımlar

- ~~`globalMediaControl` capability'si paketlenmemiş bir Win32 exe'de gerekiyor mu?~~
  **Hayır.** Manifest yok, bildirim yok, UAC yok; `RequestAsync` soğukta 30 ms,
  sıcakta 10 ms. Sadece P5'te MSIX paketlenirse konu geri gelir.

**Yan bulgu — P2'yi doğrudan değiştiriyor:** yeni bir SMTC oturumu **boş doğuyor**
(`CurrentSessionChanged` anında başlık boş, durum `Opened`; içerik 14 ms sonra
geliyor) ve tek bir parça geçişi **811 ms'ye yayılan 8 olay** üretiyor. Dolayısıyla
`CurrentSessionChanged` tek başına kutuyu açmamalı, 150 ms'lik birleştirme penceresi
de "kutuyu aç" kararını değil "animasyonu baştan oynatma" kararını yönetmeli.
Ayrıntı ve diğer altı madde: `docs/P1-SONUC.md` § 8.

**Yan bulgu — mimariyi ilgilendiriyor:** `SW_HIDE` pencereyi Win32/DWM'den gizliyor ama
**WebView2'nin render döngüsünü durdurmuyor** — gizliyken de saniyede 185 kare üretiyor
(60 sn'de 11 098 kare). Tauri'nin `window.hide()` yolu da durdurmuyor. Direct Flip
korunuyor ama GPU boşuna çalışıyor; kutu günün çoğunu gizli geçireceği için pil ömrü
açısından çözülmeli — **P3'e alındı**, çünkü gerçek UI olmadan uyanma bedeli anlamlı
biçimde yeniden ölçülemez.

---

## Mimari kuralı — tek cümle

Windows API'leri **sadece** `adapters` katmanında yaşar. O katmanın üstünde hiçbir
yerde `unsafe` blok, WinRT tipi veya COM arayüzü görünmez. Ayrıntı:
`docs/ARCHITECTURE.md`.

---

## Dosya haritası

```
CLAUDE.md                 bu dosya
docs/ARCHITECTURE.md      katmanlar, adapter'lar, API haritası, gecikme bütçesi
docs/ROADMAP.md           P0–P5, her birinin çıkış kriteri
docs/DESIGN.md            DOM sözleşmesi, tema sistemi, token tablosu
docs/P0-SONUC.md          gecikme kapısı ölçümü, yöntem ve karar
docs/P1-SONUC.md          SMTC ölçümü: olay akışının gerçek şekli, P2'ye devredilenler
spikes/p0-tauri           P0 ölçüm düzeneği — Tauri (atılacak kod)
spikes/p0-wpf             P0 ölçüm düzeneği — WPF karşılaştırması (atılacak kod)
spikes/p1-smtc            P1 ölçüm düzeneği — SMTC konsol aracı (atılacak kod)
spikes/analyze.py         measurements/*.csv → p50/p95 tablosu
design/flyout.css         tema-bağımsız iskelet — kutu, ızgara, ölçüler
design/themes/cyber.css   cyber teması: token'lar + yapısal override'lar
design/themes/neon.css    neon teması: aynısı
design/preview.html       sekiz durumun tamamı, tarayıcıda aç
```

`design/*.css` dosyaları **üretim kodudur**, mockup değil. Uygulamanın frontend'i
bunları doğrudan kullanır. Renk, boşluk, yazı boyutu değerlerini oradan al; yeniden
uydurma, yuvarlama, "4px grid'e oturtma" yapma.

---

## Ortam — bu makinede zaten kurulu, yeniden keşfetme

| | |
|---|---|
| Rust | 1.98.0, `stable-x86_64-pc-windows-msvc` |
| MSVC | VS 2022 Build Tools (VCTools) + Windows SDK 10.0.26100 |
| .NET | SDK 8.0.424 |
| Node / npm | v24.19 / 11.17 |
| WebView2 Runtime | 151.0.4129.107 |
| Makine | Ryzen 5 7600X · RTX 5070 · 2560×1440 **@184 Hz** · Win11 build 26200 |

**PATH tuzağı:** Claude Code'un kabuğu kurulum sırasındaki PATH'i miras alıyor;
`cargo`/`dotnet` "bulunamadı" derse komutun başına şunu koy:

```powershell
$env:Path = [Environment]::GetEnvironmentVariable("Path","Machine") + ";" + [Environment]::GetEnvironmentVariable("Path","User")
```

### ⚠ Smart App Control — Rust derlemelerini engelliyor

Bu makinede SAC **açık** (`VerifiedAndReputablePolicyState = 1`). İmzasız Rust
ikililerini **hash başına, öngörülemez biçimde** engelliyor: aynı kaynaktan üretilen
derlemelerin bir kısmı çalıştı, bir kısmı `Uygulama Denetimi ilkesi bu dosyayı
engelledi` verdi. .NET etkilenmiyor (apphost bilinen şablon olduğu için geçiyor).

Engellenirse: kaynakta önemsiz bir değişiklik yapıp yeniden derle — farklı hash bazen
geçiyor. Kalıcı çözüm ikisinden biri, **ikisi de kullanıcı kararı**: SAC'ı kapatmak
(*geri alınamaz* — tekrar açmak Windows'u sıfırlamayı gerektirir) veya derlemeleri
güvenilen bir sertifikayla imzalamak. Karar verilmedi; kendi başına kapatma.

### Depo

`eemresari/neon-flyout` (public), `v1-kasa` deposuna **submodule** olarak bağlı.
Push için her zaman kullanıcı onayı iste. `measurements/*.csv` ve build çıktıları
`.gitignore`'da.

---

## Çalışma tarzı

- Emin olmadığın bir API davranışını tahmin etme. Küçük bir spike yaz, çalıştır, gör.
- Bir varsayım doğrulandığında yukarıdaki listeden çıkar ve sonucu yaz.
- P0 ve P1 birer ölçüm/spike; güzel kod yazmaya çalışma, cevabı al ve at.
- Tasarımda bir şeyi değiştirmen gerekirse `design/` içinde değiştir, koda gömme.
