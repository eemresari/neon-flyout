# P4 — native flyout bastırma: kaynak araştırması ve ilk ölçüm

**Tarih:** 2026-08-29 · **Durum: araştırma bitti, ölçüm beklentiyi çürüttü.**
Kod yazılmadı; önce cevaplanması gereken bir soru çıktı (§ 4).

Bu aşama `CLAUDE.md`'deki 1. doğrulanmamış varsayımı hedefliyor: *"Windows'un native
flyout'unu gizlemenin güncel yöntemi."* Yöntemin **ne olduğu** artık biliniyor
(§ 1–2); **bu makinede uygulanabilir olup olmadığı** bilinmiyor (§ 3).

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

---

## 4. Cevaplanması gereken soru — kod yazmadan önce

Ölçüm iki şeyden birini söylüyor ve hangisi olduğu **gözle** ayırt edilmeli:

1. **Native OSD bu makinede zaten çıkmıyor.** (Daha önce bir araçla kapatılmış
   olabilir, ya da bu derlemede davranış değişmiş.) Öyleyse P4'ün bastıracağı bir
   şey yok; aşama "tespit et, yoksa sessiz vazgeç" koduna indirgenir.
2. **Çıkıyor, ama sentetik `keybd_event` onu tetiklemiyor.** Kabuk enjekte edilmiş
   girdiyi OSD yolunda eleyebiliyor. Öyleyse ölçüm geçersiz ve **fiziksel** tuşla
   tekrarlanmalı.

`CLAUDE.md`'nin kuralı burada bağlayıcı: *tahmin etme, ölç.* İkisi arasında karar
vermeden bastırma kodu yazmak, olmayan bir pencereyi kovalamak olur.

---

## 5. Sonraki adım (soru cevaplanınca)

- **1. şıksa:** `NativeFlyoutSuppressor` yalnızca *tespit* eder ve bulamazsa sessiz
  vazgeçer; ayarlarda "native flyout bulunamadı" durumu gösterilir. P4 çıkış kriteri
  zaten bunu istiyor.
- **2. şıksa:** ölçüm fiziksel tuşla tekrarlanır, hedef pencere tespit edilir ve
  § 2'deki FluentFlyout yaklaşımı **ModernFlyouts'un bant kontrolüyle birlikte**
  uygulanır: `band == 0x12` şartı olmadan asla, yoksa Alt+Tab kurban gider.
- Her iki durumda da bastırma `adapters/` katmanında yaşar (`unsafe`/Win32 kuralı),
  `core/` bundan haberdar olmaz.

---

## Kaynaklar

- ModernFlyouts — `ModernFlyouts.Core/Interop/NativeFlyoutHandler.cs`,
  `ModernFlyouts.Core/Interop/BandWindow.cs` (ZBandID enum),
  PR #1077 (22H2 sınıf değişimi)
- FluentFlyout — `FluentFlyoutWPF/Windows/VolumeMixerWindow.xaml.cs`
  (`HideVolumeOsd` / `ShowVolumeOsd`), kaynağı `gpkgpk/HideVolumeOSD`
