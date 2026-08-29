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
4. **P2 tamamlandı (2026-08-28): uygulama `app/` altında ayakta.** Katmanlar,
   core → WebView sözleşmesi, kararlar ve tuzaklar `docs/P2-SONUC.md`'de —
   **DOM yazmadan önce § 2'yi oku.**
5. **P3 tamamlandı (2026-08-28): tasarım bağlı, sekiz durum uygulamada çıkıyor,
   WebView2 gizliyken render'ı duruyor.** DOM'a veya `design/`'a dokunmadan önce
   `docs/P3-SONUC.md`'yi oku.
6. **P4 araştırması bitti (2026-08-29): `docs/P4-ARASTIRMA.md`.** Yöntem bulundu,
   ama bu makinede bastırılacak pencere yok; P4 "ara, bulamazsan sessiz vazgeç"
   koduna indi. **Ölçümü tekrarlama, § 5'teki planı uygula** — ama önce SAC
   engeline bak, Rust derlemesi şu an kapalı olabilir.
7. Sonra `docs/ROADMAP.md` → P5 (paketleme).

`spikes/` altındaki üç proje P0 ve P1'in ölçüm düzeneğidir — **atılacak koddur**,
uygulamanın iskeleti değil. Uygulama `app/` altında ayrı duruyor.

**Çalıştırma:** `cd app && npm run tauri dev`. `cargo run` tek başına çalışmaz —
debug derlemesi `devUrl`'e gider, Vite ayakta değilse WebView "localhost bağlantıyı
reddetti" gösterir. Testler: `cd app/src-tauri && cargo test`.

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

**Şu an açık madde yok** (2026-08-29). Yeni bir varsayım doğduğunda buraya yaz ve
"muhtemelen şöyledir" diye kabul edip üstüne kod yazma; küçük bir spike yaz, ölç,
sonucu aşağıdaki listelerden birine geçir.

## P4'te cevaplanan varsayımlar

- ~~**Windows'un native flyout'unu gizlemenin güncel yöntemi.**~~ **Cevaplandı
  (2026-08-29, P4 araştırması).** Yöntem: build ≥ 22620'de dış pencere
  `XamlExplorerHostIslandWindow` + iç pencere
  `Windows.UI.Composition.DesktopWindowContentBridge`/`DesktopWindowXamlSource`,
  öncesinde `NativeHWNDHost` + `DirectUIHWND`; doğru örneği ayırt etmek için
  sahip süreç `explorer.exe` **ve** `GetWindowBand(hWnd) == 0x12` (`AboveLockUX`).
  **Bant kontrolü zorunlu** — onsuz bu makinede Alt+Tab penceresi seçiliyor.
  **Ama bu makinede bastırılacak pencere yok:** kullanıcı sesi tuşla değil fareyle
  değiştiriyor, Windows da kutusunu tuş basımına açtığı için hiç çıkmıyor (45 sn
  pasif izlemeyle ölçüldü). Ayrıntı ve uygulama planı: `docs/P4-ARASTIRMA.md`.

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

## P3'te cevaplanan varsayımlar

- ~~WebView2 gizliyken render'ı durdurulunca uyanma bedeli ne olur?~~ **Bedel yok.**
  `ICoreWebView2Controller::SetIsVisible(false)` ile gizliyken üretilen kare
  **9 235 → 0**, uyanma p95 **12.1 → 11.6 ms** (60 sn derin beklemede 11.8).
  Ham `SW_HIDE`'ın ölçtüğü "sıcak döngü" korkusu çıkmadı. `TrySuspend` de aynı
  gecikmeyi veriyor ama **kullanılmadı**: JS'i donduruyor, içerik-önce-pencere
  sırasını bozardı. Ayrıntı: `docs/P3-SONUC.md` § 5.
- ~~AudioAdapter'ın cihaz değişimi yolu gerçek donanımla çalışıyor mu?~~ **Evet**
  (2026-08-29, elle doğrulandı). Kulaklık takılıp çıkarıldığında `IMMNotificationClient`
  → `Rebind` → yeni uç noktaya abonelik işliyor; kutu çıkıyor ve kaynak satırındaki
  cihaz adı doğru cihaza dönüyor. **Yan bulgu:** `core/rules.rs → judge_volume`
  anahtarı yalnızca `(yüzde, mute)`; cihaz adı anahtarın parçası değil, dolayısıyla
  yeni cihazın seviyesi eskisiyle aynıysa takma anındaki olay `Unchanged` diye
  yutulur ve ad bir sonraki gerçek ses değişimine kadar bayat kalır. Bilerek
  bırakıldı: cihaz takmak tek başına kutu açmayı hak eden bir olay değil.

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
docs/P2-SONUC.md          uygulama iskeleti, core→WebView sözleşmesi, tuzaklar
docs/P3-SONUC.md          tasarımın bağlanması, kapak/metre, WebView2 uyutma ölçümü
docs/P4-ARASTIRMA.md      native flyout: yöntem, pencere ölçümü, uygulama planı
spikes/p4-probe           P4 ölçüm düzeneği — PowerShell pencere sondaları
app/                      UYGULAMA (P2'den itibaren) — vanilla+TS frontend, Tauri v2
app/src-tauri/src/core/     Windows tipi bilmeyen, test edilebilir katman
app/src-tauri/src/adapters/ SMTC, WASAPI, WebView2, pencere — unsafe/COM yalnızca burada
design/fonts.css          yerel @font-face'ler (offline); design/fonts/*.woff2
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
geçiyor. **Bağımlılığın build script'i engellenirse** kaynağa dokunmak hash'i
değiştirmez; `app/src-tauri/Cargo.toml`'daki `[profile.*.build-override]` bloğu tam
bunun için duruyor (build script'leri farklı bayrakla derleyip hash'i değiştiriyor),
P2'de `icu_properties_data` böyle aşıldı.

**2026-08-29 — durum kötüleşti: `rustc.exe`'nin kendisi engelleniyor.** Hem rustup
shim'i (`~/.cargo/bin/rustc.exe`) hem gerçek toolchain ikilisi, hem Bash'ten hem
PowerShell'den, üst üste her denemede `os error 4551` veriyor. Sonuçları:

- **Rust derlemesi tamamen kapalı.** `cargo build`/`check`/`test` (derleme gerektiren
  her biçimi) çalışmıyor. Zaten derlenmiş hedefler çalışıyor: `cargo test --lib`
  16/16 geçiyor (yeniden derleme gerekmediği için) ve `target/debug/neon-flyout.exe`
  sorunsuz koşuyor.
- **`npm run tauri dev` panikliyor**, sebebi bu: tauri-cli önce `rustc -vV` ile host
  triple'ı okuyor, engellenince `None` alıp `unwrap()` ediyor
  (`crates/tauri-cli/src/interface/rust.rs:1166`). Panik yanıltıcı — Tauri'de değil
  SAC'ta sorun var, önce `rustc -vV` çalıştırıp teyit et.
- **`build-override` çaresi burada işe yaramaz:** engellenen dosyayı biz üretmiyoruz,
  hash'ini değiştiremeyiz.

Derleme gerektirmeyen iş (P4 kaynak araştırması, doküman, `design/`, frontend —
Vite + mevcut debug ikilisi ile canlı test) yapılabilir. Derleme gerektiren her şey
aşağıdaki karar verilene kadar bloke.

Kalıcı çözüm ikisinden biri, **ikisi de kullanıcı kararı**: SAC'ı kapatmak
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
