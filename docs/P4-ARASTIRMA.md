# P4 — native flyout bastırma: kaynak araştırması ve ilk ölçüm

**Tarih:** 2026-08-29 · **Durum: araştırma ve ölçüm bitti, kod yazılmadı (SAC bloke).**

Bu aşama `CLAUDE.md`'deki 1. doğrulanmamış varsayımı hedefliyor: *"Windows'un native
flyout'unu gizlemenin güncel yöntemi."* **Cevap alındı:** yöntem § 1–2'de, ama
ölçüm (§ 3) bu makinede bastırılacak bir pencere olmadığını gösterdi — gerekçesi
projenin kendi varlık sebebiyle aynı (§ 4). Uygulama planı § 5'te; Rust derlemesi
Smart App Control tarafından engellendiği için yazılamadı.

---

## 1. Hedef pencere — iki bağımsız uygulamanın ortak cevabı

İki depo da aynı pencere çiftini arıyor, sürüme göre sınıf adı değişiyor:

| Windows | Dış pencere (sınıf / başlık) | İç pencere (sınıf / başlık) |
|---|---|---|
| 11 22H2+ (build ≥ 22620) | `XamlExplorerHostIslandWindow` / `""` | `Windows.UI.Composition.DesktopWindowContentBridge` / `DesktopWindowXamlSource` |
| 10 ve öncesi | `NativeHWNDHost` / `""` | `DirectUIHWND` / `""` |

Kaynak: ModernFlyouts `ModernFlyouts.Core/Interop/NativeFlyoutHandler.cs → GetAllInfos()`
(PR #1077 eşiği `> 22000` yazıyordu, **main'deki hâli `>= 22620`** — geçerli olan bu).

### Ayırt etme — asıl zor kısım

`XamlExplorerHostIslandWindow` kabuğun **genel amaçlı** XAML ada penceresi; tek
kullanıcısı flyout değil. İki proje bu yüzden iki farklı eleme uyguluyor:

- **ModernFlyouts (kesin yöntem):** sahibi `explorer.exe` **ve**
  `GetWindowBand(hWnd) == ZBandID.AboveLockUX (0x12)`. `GetWindowBand`
  belgelenmemiş bir `user32.dll` ihracı; z-band'i okur.
- **FluentFlyout (sezgisel):** `Windows.UI.Input.InputSite.WindowClass` torun
  penceresi var mı, ve `GetWindowRect` sıfır değil mi. Bant kontrolü yok.
  (`FluentFlyoutWPF/Windows/VolumeMixerWindow.xaml.cs → HideVolumeOsd()`,
  kendisi `gpkgpk/HideVolumeOSD`'den türetilmiş.)

**§ 3'teki ölçüm bu ikisinin arasındaki farkı kritik hâle getiriyor** — FluentFlyout'un
sezgiseli bu makinede **yanlış pencereyi** seçerdi.

---

## 2. Gizleme mekanizması — iki ayrı yaklaşım

### ModernFlyouts: sürekli, olay güdümlü

```csharp
ShowWindowAsync(HWndDUI,  SW_MINIMIZE);
ShowWindowAsync(HWndHost, SW_HIDE);
```

Her gösterimde yeniden uygulanıyor. Tetik: kabuk sürecine kurulan
`SetWinEventHook(EVENT_OBJECT_CREATE … EVENT_OBJECT_STATECHANGE, shellPid,
WINEVENT_OUTOFCONTEXT)`; `EVENT_OBJECT_SHOW` gelince gizle, `EVENT_OBJECT_DESTROY`
gelince durumu sıfırla ve 3 sn'lik zamanlayıcıyla yeniden kancala. Yani **flyout
penceresi kalıcı değil: gösterilince yaratılıyor, sonra yok ediliyor.**

### FluentFlyout: tek seferlik, geri alınabilir

```csharp
SetWindowLong(host, GWL_EXSTYLE, orijinal | WS_EX_LAYERED | WS_EX_TRANSPARENT);
SetWindowPos(host, 0, -99999, -99999, 0, 0, SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE);
ShowWindow(host, SW_MINIMIZE);
```

Pencereyi ekran dışına taşıyıp tıklama geçirgen yapıyor. Çıkarken `ShowVolumeOsd()`
orijinal ex-style'ı, konumu ve `SW_RESTORE`'u geri koyuyor. Kanca yok, sürekli iş yok.

**Bizim için doğru olan bu ikincisi** — kanca kurmadan, tek seferde, çıkışta geri
alınabilir; P4'ün çıkış kriteri ("bastırma başarısız olsa bile uygulama çalışmaya
devam etsin") ile de örtüşüyor. Ama önce § 4'teki soru cevaplanmalı.

---

## 3. Ölçüm — bu makinede (build 26200) flyout penceresi **yok**

Salt-okunur PowerShell sondaları (`spikes/p4-probe/`), Rust derlemesi gerektirmiyor
(SAC engeli için `CLAUDE.md`'ye bak).

**3.1 — Sınıf taraması.** Tüm üst düzey pencereler tarandı:

```
XamlExplorerHostIslandWindow : 1 adet
  pid=explorer  band=16  görünür=False  başlık='Görev Geçişi'
  rect=0,0 2560x1392   bridge=VAR  InputSite=VAR
NativeHWNDHost : 0 adet
```

Tek örnek **Alt+Tab** penceresi: band **16** (`SystemTools`), ModernFlyouts'un
aradığı **18** (`AboveLockUX`) değil.

> ⚠ **FluentFlyout'un sezgiseli bu makinede Alt+Tab penceresini seçerdi**
> (bridge + InputSite + sıfır olmayan rect — üç şart da tutuyor) ve onu ekran
> dışına taşıyıp küçültürdü. Bant kontrolü olmadan bu yöntem **kullanılamaz.**
> ModernFlyouts'un `band == 0x12` şartı doğru davranıyor: hiçbir şey bulmuyor.

**3.2 — Ses tuşu anında pencere farkı.** `VOLUME_UP` gönderildi, 423 üst düzey
pencere 2 saniye boyunca 60 ms'de bir örneklendi (yeni pencere / görünürlük / konum
değişimi):

```
GORUNURLUK False->True  [Tauri Window] pid=neon-flyout  band=1
```

**Tek olay bizim kutumuz.** Windows hiçbir pencere yaratmadı, göstermedi, kıpırdatmadı.

**3.3 — Kabuk süreçlerinin pencere dökümü.** `explorer`, `shellhost`, `dwm`, `sihost`
vb. süreçlere ait 66 üst düzey pencerenin tamamı listelendi. OSD host'una benzeyen
tek aday yok; band 18'de hiçbir pencere yok.

**3.4 — Pasif izleme, gerçek kullanıcı akışıyla.** Yukarıdaki ölçümler sentetik
`keybd_event` kullanıyordu; kullanıcı **ses tuşunu hiç kullanmıyor, sesi fareyle
değiştiriyor.** Dolayısıyla yanlış yol test edilmişti. `p4-passive.ps1` hiçbir girdi
göndermeden 45 saniye boyunca 437 pencereyi 50 ms'de bir örnekledi; kullanıcı bu
sırada sesi kendi normal yoluyla değiştirdi:

```
23:56:31.082  YENI PENCERE  [XamlExplorerHostIslandWindow] explorer band=1     ← snap layout
23:56:34.135  YENI PENCERE  [Xaml_WindowedPopupClass] explorer 'PopupHost'
23:56:36.822  GORUNURLUK False->True   [Tauri Window] neon-flyout              ← SES DEGISIMI
23:56:39.479  GORUNURLUK True->False   [Tauri Window] neon-flyout              ← 2.6 sn sonra
```

Ses değiştiği anda (36.822) **hiçbir Windows penceresi yaratılmadı, gösterilmedi,
kıpırdamadı.** 31. saniyedeki ada penceresi ses değil pencere sürüklemeye ait (aynı
anda TouchDesigner penceresi taşınıyordu, band 1 = Desktop, saniyeler içinde yok
oldu — sonradan alınan tarama yine tek `XamlExplorerHostIslandWindow` görüyor:
Alt+Tab, band 16).

---

## 4. Sonuç — bu makinede bastırılacak bir şey yok

**Native OSD bu kullanıcının akışında hiç çıkmıyor**, ve sebebi projenin kendi
varlık gerekçesiyle aynı: Windows'un flyout'u **ses/medya tuşuna basılmasına** tepki
veriyor, **ses seviyesinin değişmesine** değil. Kullanıcı sesi fareyle değiştiriyor,
dolayısıyla ortada basılmış bir tuş yok — Windows da kutusunu açmıyor. `CLAUDE.md`
→ *"Neden bu uygulama var"* bölümünün ses tarafındaki karşılığı tam olarak bu.

Sentetik `keybd_event` ile de OSD çıkmadı (§ 3.2); kabuk enjekte edilmiş girdiyi
eliyor olabilir, ama bu artık **önemsiz** — gerçek akış zaten tuş kullanmıyor.

**P4 buna göre küçülüyor:** bastırma bir *özellik* değil, bir *emniyet kemeri*.
Kod, flyout penceresini arar; bulursa bastırır, bulamazsa sessizce vazgeçer — ki bu
makinede her zaman ikincisi olacak. Yol haritasının çıkış kriteri (*"bastırma
başarısız olduğunda uygulama çalışmaya devam ediyor"*) zaten tam olarak bunu istiyor.

---

## 5. Uygulama planı

`adapters/native_flyout.rs` — `unsafe`/Win32 kuralı gereği `adapters/` içinde;
`core/` bundan haberdar olmaz.

1. **Tespit** (`ModernFlyouts` yöntemi, birebir):
   `FindWindowEx` döngüsüyle build'e göre dış sınıfı gez → iç pencereyi doğrula →
   sahip süreç `explorer.exe` mi → **`GetWindowBand(hWnd) == 0x12` (`AboveLockUX`)**.
   **Bant kontrolü pazarlık konusu değil:** § 3.1'de ölçüldüğü gibi, o şart olmadan
   FluentFlyout'un sezgiseli bu makinede Alt+Tab penceresini seçer ve ekran dışına
   taşır.
2. **Bastırma** (`FluentFlyout` yöntemi): `WS_EX_LAYERED | WS_EX_TRANSPARENT` ekle,
   `SetWindowPos(-99999, -99999)`, `SW_MINIMIZE`. Orijinal ex-style saklanır.
3. **Geri alma:** çıkışta ex-style, konum ve `SW_RESTORE` geri konur. Uygulama
   kapanınca kullanıcının masaüstünde kalıcı bir değişiklik bırakmıyoruz.
4. **Bulamazsa:** tek satır günlük, `Ok(())`. Yeniden deneme yok, kanca yok,
   zamanlayıcı yok — ModernFlyouts'un 3 sn'lik rehook döngüsü bize gereksiz;
   o, bastırmanın *çalışmasına* bağımlı bir uygulama, biz değiliz.
5. **Doğrulama:** `p4-probe.ps1` bastırma öncesi/sonrası çalıştırılır. Bu makinede
   beklenen sonuç "hiçbir şey bulunamadı, uygulama normal çalışıyor" — çıkış
   kriterinin kendisi bu.

### Durum: kod yazıldı, **derlenmedi**

`app/src-tauri/src/adapters/native_flyout.rs` yukarıdaki planı uyguluyor ama
**`adapters/mod.rs`'e bağlanmadı** — yani derlemeye hiç girmiyor. Sebep: Smart App
Control (§ 6). Engel kalkınca sırayla:

1. `adapters/mod.rs` → `pub mod native_flyout;`
2. `cargo test --lib` (derleme + mevcut 16 test)
3. `lib.rs` `setup`'ında `native_flyout::suppress()`, çıkışta `restore()`
4. `spikes/p4-probe/p4-probe.ps1` ile öncesi/sonrası karşılaştır

Bu makinede beklenen sonuç `suppress() == false` ve uygulamanın normal çalışması —
çıkış kriterinin kendisi bu.

---

## 6. Neden derlenemedi — SAC denemelerinin dökümü

`CLAUDE.md` → SAC bölümü durumu anlatıyor; bu oturumda denenenler ve sonuçları:

| Deneme | Sonuç |
|---|---|
| `rustup update stable` | **Boşa gitti.** rustc'nin sürümünü okuyamadığı için "unchanged" deyip hiç indirmedi. |
| Olay günlüğü (`Microsoft-Windows-CodeIntegrity/Operational`) | Engel netleşti: olay 3033/3077, `~/.cargo/bin/rustc.exe` → toolchain `rustc.exe` yüklenemiyor, *"did not meet the Enterprise signing level requirements"*, Policy `{0283ac0f-…}`. |
| `beta` toolchain kurulumu (1.99.0-beta.2) | **Kısmi kazanç.** Farklı hash → beta `rustc.exe` 3/3 çalışıyor. |
| İkili bazında tarama | stable `cargo.exe` ✅ 3/3 · stable `rustc.exe` ❌ 3/3 · beta `cargo.exe` ❌ 3/3 · beta `rustc.exe` ✅ 3/3. **Engellenen `~/.cargo/bin` shim'leri ve stable rustc; çalışan kombinasyon stable cargo + beta rustc.** |
| `RUSTC=<beta> <stable cargo> test --lib`, üç farklı `build-override` bayrak kümesiyle | **Üçü de başarısız**, ama her seferinde *farklı* bir build script'te (`getrandom`, `quote`, yine `getrandom` — farklı hash). ~20-40 crate derlendikten sonra düşüyor. |

**Sonuç:** bayrak değiştirmek build script'lerin hash'ini değiştiriyor ama hepsini
birden geçirmiyor; her deneme yeni bir kura. Yakınsamıyor. Geriye iki gerçek yol
kalıyor ve **ikisi de kullanıcı kararı**: SAC'ı kapatmak (*geri alınamaz*) veya
derlemeyi GitHub Actions'ta yapmak (depo zaten public).

---

## Kaynaklar

- ModernFlyouts — `ModernFlyouts.Core/Interop/NativeFlyoutHandler.cs`,
  `ModernFlyouts.Core/Interop/BandWindow.cs` (ZBandID enum),
  PR #1077 (22H2 sınıf değişimi)
- FluentFlyout — `FluentFlyoutWPF/Windows/VolumeMixerWindow.xaml.cs`
  (`HideVolumeOsd` / `ShowVolumeOsd`), kaynağı `gpkgpk/HideVolumeOSD`
