# Overlay Flyout — proje brifingi

Windows için masaüstü uygulaması. Sistem medya oturumunu okur; parça değiştiğinde,
çalma durumu değiştiğinde veya ses ayarlandığında ekranın sağ üst köşesinde ~2.5
saniyelik bir kutu gösterip kaybolur. Windows'un kendi medya/ses flyout'unun yerine
geçer. Kendi çalma motoru **yok**.

Hedef makine: Windows 11 x64. Minimum: Windows 10 1809 (build 17763).

---

## İlk oturumda sırayla yapılacaklar

1. Bu dosyayı ve `docs/` altındaki üç dosyayı oku.
2. **`docs/ROADMAP.md` → P0'ı çalıştır.** P0 bir ölçüm, kod değil, ve sonucu yığın
   seçimini değiştirebilir. P0 geçmeden P1'e başlama.
3. P0 geçerse P1 (SMTC spike), sonra sırayla devam.

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

**P0 gecikme kapısı geçmezse yedek plan: .NET 8 + WPF.** Karar P0'da ölçümle verilir,
tartışmayla değil.

Proje iskeleti şu komutla kurulur — elle uydurma:

```bash
npm create tauri-app@latest
```

---

## Doğrulanmamış varsayımlar — tahmin etme, ölç

Bunlar bilerek açık bırakıldı. Herhangi birini "muhtemelen şöyledir" diye kabul edip
üstüne kod yazma; küçük bir spike yaz, ölç, sonucu buraya yaz.

1. **Gizli bir WebView2 penceresi `SW_SHOW`'dan sonra kaç ms'de ilk kareyi basar?**
   Bütçe 50 ms. Bilinmiyor. Tüm yığın kararı buna bağlı → P0.
2. **Windows'un native flyout'unu gizlemenin güncel yöntemi.** Belgelenmemiş; sürümden
   sürüme değişiyor (`XamlExplorerHostIslandWindow`, `NativeHWNDHost` isimleri geçiyor
   ama hangisinin doğru olduğu doğrulanmadı). Kaynak: FluentFlyout ve ModernFlyouts
   depolarındaki güncel handler kodu.
3. **`globalMediaControl` capability'si** paketlenmemiş bir Win32 exe'de gerekiyor mu?
   Muhtemelen hayır, ama test edilmedi. Sadece MSIX paketlersen sorun olur.
4. **Şeffaf Tauri penceresinde `backdrop-filter: blur()`** Windows'ta çalışıyor mu?
   macOS'ta bozulduğu raporlanmış. Tasarımlar blur'a bağımlı değil, ama denenebilir.

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
design/flyout.css         tema-bağımsız iskelet — kutu, ızgara, ölçüler
design/themes/cyber.css   cyber teması: token'lar + yapısal override'lar
design/themes/neon.css    neon teması: aynısı
design/preview.html       sekiz durumun tamamı, tarayıcıda aç
```

`design/*.css` dosyaları **üretim kodudur**, mockup değil. Uygulamanın frontend'i
bunları doğrudan kullanır. Renk, boşluk, yazı boyutu değerlerini oradan al; yeniden
uydurma, yuvarlama, "4px grid'e oturtma" yapma.

---

## Çalışma tarzı

- Emin olmadığın bir API davranışını tahmin etme. Küçük bir spike yaz, çalıştır, gör.
- Bir varsayım doğrulandığında yukarıdaki listeden çıkar ve sonucu yaz.
- P0 ve P1 birer ölçüm/spike; güzel kod yazmaya çalışma, cevabı al ve at.
- Tasarımda bir şeyi değiştirmen gerekirse `design/` içinde değiştir, koda gömme.
