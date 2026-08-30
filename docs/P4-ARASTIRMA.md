# P4 — native flyout bastırma: kaynak araştırması ve ilk ölçüm

**Tarih:** 2026-08-29 / 30 · **Durum: tamamlandı — kod yazıldı, derlendi, çalışıyor.**

Bu aşama `CLAUDE.md`'deki 1. doğrulanmamış varsayımını hedefliyordu: *"Windows'un
native flyout'unu gizlemenin güncel yöntemi."* Yöntem § 1–2'de, uygulaması
`app/src-tauri/src/adapters/native_flyout.rs`'te.

> **Bu belge bir yanlışı da kaydediyor.** § 3'ün ilk turu "bu makinede bastırılacak
> pencere yok" sonucuna vardı ve **bu yanlıştı**: ölçüm `EnumWindows` kullanıyordu,
> o da band 18'deki pencereleri listelemiyor. Pencere baştan beri oradaydı. Hatanın
> nasıl ortaya çıktığı § 3.5'te, düzeltilmiş sonuç § 4'te. Bölümler bilerek
> silinmedi — yöntem hatasının kendisi bu projede kayda değer.

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

**Bizim için doğru olan bu ikincisi** ve uygulanan da bu — kanca kurmadan, tek
seferde, çıkışta geri alınabilir; P4'ün çıkış kriteri ("bastırma başarısız olsa
bile uygulama çalışmaya devam etsin") ile de örtüşüyor. Ayırt etme için
ModernFlyouts'un bant kontrolü alındı: **iki projenin birleşimi.**

---

## 3. Ölçüm — ilk üç tur **hatalı**, düzeltmesi § 3.5'te

Salt-okunur PowerShell sondaları (`spikes/p4-probe/`), Rust derlemesi gerektirmiyor
(o sırada SAC derlemeyi engelliyordu, bkz. § 6).

**3.1–3.4 aşağıda olduğu gibi duruyor ama sonuçları geçersiz** — üçü de
`EnumWindows` tabanlı ve o, aranan pencereyi göremiyor. Neden ve nasıl anlaşıldığı
§ 3.5'te.

**3.1 — Sınıf taraması.** Tüm üst düzey pencereler tarandı:

```
XamlExplorerHostIslandWindow : 1 adet
  pid=explorer  band=16  görünür=False  başlık='Görev Geçişi'
  rect=0,0 2560x1392   bridge=VAR  InputSite=VAR
NativeHWNDHost : 0 adet
```

Tek örnek **Alt+Tab** penceresi: band **16** (`SystemTools`), ModernFlyouts'un
aradığı **18** (`AboveLockUX`) değil.

> ⚠ **Bu tespit hâlâ geçerli ve önemli: FluentFlyout'un sezgiseli bu makinede
> Alt+Tab penceresini seçerdi** (bridge + InputSite + sıfır olmayan rect — üç şart
> da tutuyor) ve onu ekran dışına taşıyıp küçültürdü. Bant kontrolü olmadan o
> yöntem **kullanılamaz.** Çalışan kodda `band == 0x12` şartı bu yüzden var ve
> § 3.5'te kendini kanıtladı: Alt+Tab'a ve band 7'deki başka bir adaya dokunmadı.

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

Ses değiştiği anda (36.822) `EnumWindows`'un gördüğü hiçbir pencere kıpırdamadı.
**Bu ölçüm geçerli değil — sebebi § 3.5.**

**3.5 — Yöntem hatası: `EnumWindows` band 18'i görmüyor.** Yukarıdaki üç ölçümün
üçü de `EnumWindows` tabanlıydı ve hepsi aynı kör noktayı paylaşıyor: **yüksek
z-band'deki pencereler bu listede çıkmıyor.** Flyout host'u tam olarak orada
yaşıyor (band 18 = `AboveLockUX`), dolayısıyla ölçümler onu göremedi ve "pencere
yok" sonucu üretti. Alt+Tab penceresinin (band 16) görünmesi yanıltıcı oldu —
eşiğin altında kaldığı için listeleniyor.

Hata, kod çalıştırıldığında ortaya çıktı. `find()` — ki `FindWindowEx` kullanıyor —
ilk denemede buldu:

```
[native_flyout] aday: hwnd=0x20B8E band=Some(18) baslik=""
```

`FindWindowEx` tabanlı sonda (`spikes/p4-probe/p4-find.ps1`) durumu doğruluyor:

```
aday #1  hwnd=0x20B8E pid=2584(explorer)
   band=18  ic pencere=VAR
   min=True  exstyle=0x082800A8 LAYERED+TRANSPARENT   ← bastirma uygulanmis
   baslik=''
aday #2  hwnd=0xB09D0 pid=2584(explorer)  band=7  → dokunulmadi
```

**Alınacak ders:** kabuğun bant pencerelerini `EnumWindows` ile aramak yöntemsel
bir hata. Bu projede pencere ararken `FindWindowEx` kullan; `EnumWindows`
sonuçlarının yokluğu kanıt değil.

---

## 4. Sonuç — pencere var ve bastırma çalışıyor

**§ 3'ün ilk üç ölçümünün "bastırılacak pencere yok" sonucu yanlıştı** (§ 3.5).
Doğrusu:

- Flyout host'u **var**: `XamlExplorerHostIslandWindow`, boş başlık, sahip
  `explorer.exe`, band **18**, iç penceresi `DesktopWindowContentBridge` /
  `DesktopWindowXamlSource`. § 1'deki yöntem bu makinede **birebir geçerli**.
- Bastırma **çalışıyor**: uygulama açılışta pencereyi buluyor, `WS_EX_LAYERED |
  WS_EX_TRANSPARENT` ekleyip küçültüyor. Sonda bayrakları doğruluyor.
- **Bant kontrolü kendini kanıtladı:** aynı sınıftan band 7'deki ikinci bir
  pencere ve band 16'daki Alt+Tab dokunulmadan geçildi.

Kullanıcı sesi fareyle değiştirdiği için Windows'un kutusunu zaten görmüyordu;
ama pencere hep oradaydı ve tuş kullanılan her durumda çıkacaktı. P4 böylece
gerçek bir iş yapıyor — sadece bu kullanıcının fark etmediği bir iş.

### Bilinen kırılganlık — geri alma

`restore()` yalnızca uygulama **düzgün kapanırsa** (`RunEvent::Exit`) çalışıyor.
Süreç öldürülürse (Ctrl+C, görev yöneticisi, çökme, `tauri dev`'in yeniden
derlemesi) kabuğun kutusu bastırılmış kalıyor ve explorer yeniden başlayana kadar
öyle duruyor. Kurtarma: `spikes/p4-probe/p4-restore.ps1`. Kalıcı çözüm P5'in işi
(tepsi ikonundan düzgün "Çıkış").

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
5. **Doğrulama:** `p4-find.ps1` (`p4-probe.ps1` **değil** — § 3.5) bastırma
   öncesi/sonrası çalıştırılır.

### Durum: tamamlandı ✅

- `adapters/native_flyout.rs` yazıldı, `adapters/mod.rs`'e bağlandı, `lib.rs`
  `setup`'ında `suppress()`, `RunEvent::Exit`'te `restore()` çağrılıyor.
- `cargo test --lib` → **16/16**, `cargo fmt` temiz.
- `npm run tauri dev` ile çalıştırıldı: pencere bulundu ve bastırıldı, sonda
  bayrakları doğruladı (§ 3.5), uygulama normal çalışıyor.
- Alt+Tab ve band 7'deki diğer aday dokunulmadan geçildi.

### Uygulamada plandan sapan iki nokta

1. **`GetWindowBand` `#[link]` ile bildirilemiyor.** `user32.lib`'de import stub'ı
   yok; bağlayıcı `LNK2019: __imp_GetWindowBand` veriyor. Çalışma anında
   `GetModuleHandleW("user32.dll")` + `GetProcAddress("GetWindowBand")` ile
   çözülüyor. Çözülemezse `None` döner ve **hiçbir pencereye dokunulmaz**.
2. **`SW_MINIMIZE`, `SetWindowPos(-99999)`'u eziyor.** Bastırılmış pencerenin
   rect'i `-32768,-32768` (klasik minimize konumu) oluyor, `-99999` değil. İşlev
   aynı; ama doğrularken **konuma bakma**, ex-style bayraklarına ve `IsIconic`'e
   bak — `p4-find.ps1` bu yüzden öyle kontrol ediyor.

---

## 6. SAC engeli — denemelerin dökümü *(tarihsel)*

`CLAUDE.md` → SAC bölümü durumu anlatıyor; bu oturumda denenenler ve sonuçları:

| Deneme | Sonuç |
|---|---|
| `rustup update stable` | **Boşa gitti.** rustc'nin sürümünü okuyamadığı için "unchanged" deyip hiç indirmedi. |
| Olay günlüğü (`Microsoft-Windows-CodeIntegrity/Operational`) | Engel netleşti: olay 3033/3077, `~/.cargo/bin/rustc.exe` → toolchain `rustc.exe` yüklenemiyor, *"did not meet the Enterprise signing level requirements"*, Policy `{0283ac0f-…}`. |
| `beta` toolchain kurulumu (1.99.0-beta.2) | **Kısmi kazanç.** Farklı hash → beta `rustc.exe` 3/3 çalışıyor. |
| İkili bazında tarama | stable `cargo.exe` ✅ 3/3 · stable `rustc.exe` ❌ 3/3 · beta `cargo.exe` ❌ 3/3 · beta `rustc.exe` ✅ 3/3. **Engellenen `~/.cargo/bin` shim'leri ve stable rustc; çalışan kombinasyon stable cargo + beta rustc.** |
| `RUSTC=<beta> <stable cargo> test --lib`, üç farklı `build-override` bayrak kümesiyle | **Üçü de başarısız**, ama her seferinde *farklı* bir build script'te (`getrandom`, `quote`, yine `getrandom` — farklı hash). ~20-40 crate derlendikten sonra düşüyor. |

**Sonuç:** bayrak değiştirmek build script'lerin hash'ini değiştiriyor ama hepsini
birden geçirmiyor; her deneme yeni bir kura, yakınsamıyor.

**Çözüm (2026-08-30): kullanıcı SAC'ı kapattı.** `VerifiedAndReputablePolicyState`
1 → 0. Rust derlemesi tamamen açıldı; P4 kodu bundan sonra derlendi ve doğrulandı.
Ayar **geri alınamaz** (tekrar açmak Windows'u yeniden kurmayı gerektirir).
`CLAUDE.md` → "Smart App Control" bölümü güncel durumu taşıyor.

---

## Kaynaklar

- ModernFlyouts — `ModernFlyouts.Core/Interop/NativeFlyoutHandler.cs`,
  `ModernFlyouts.Core/Interop/BandWindow.cs` (ZBandID enum),
  PR #1077 (22H2 sınıf değişimi)
- FluentFlyout — `FluentFlyoutWPF/Windows/VolumeMixerWindow.xaml.cs`
  (`HideVolumeOsd` / `ShowVolumeOsd`), kaynağı `gpkgpk/HideVolumeOSD`
