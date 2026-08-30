# P4 ölçüm düzeneği — native flyout penceresini ara

Salt-okunur. Hiçbir pencereyi gizlemez, taşımaz, değiştirmez. Rust gerektirmez —
Smart App Control `rustc`'yi engellediği için bilerek PowerShell (bkz. `CLAUDE.md`).

> ⚠ **Önce bunu oku: `EnumWindows` band 18'deki pencereleri listelemiyor.**
> `p4-probe.ps1`, `p4-diff.ps1` ve `p4-watch.ps1` üçü de `EnumWindows` tabanlı,
> dolayısıyla **native flyout host'unu göremezler** — ilk ölçüm turu bu yüzden
> "pencere yok" sanmıştı. Flyout host'unu görmek için `FindWindowEx` kullanan
> **`p4-find.ps1`**'i çalıştır. Diğer üçü genel pencere gözlemi için hâlâ işe yarar.

| Betik | Ne yapar |
|---|---|
| **`p4-find.ps1`** | **Doğru sonda.** `FindWindowEx` ile flyout host'unu bulur: band, iç pencere, ex-style bayrakları, bastırılmış mı |
| `p4-restore.ps1` | Bastırmayı **elle geri alır**. Uygulama öldürülürse (düzgün kapanmazsa) `restore()` çalışmaz; bu betik kurtarır |
| `p4-probe.ps1` | `XamlExplorerHostIslandWindow` ve `NativeHWNDHost` örneklerini listeler: sahip süreç, z-band, alt pencereler, rect — **band 18'i göremez** |
| `p4-diff.ps1` | `VOLUME_UP` gönderir, 350 ms sonra **görünür hâle gelen** pencereleri yazar, sesi geri alır |
| `p4-watch.ps1` | `VOLUME_UP` gönderir, 2 sn boyunca 60 ms'de bir **tüm** üst düzey pencereleri örnekler; yeni pencere / görünürlük / konum değişimlerini raporlar |

```powershell
powershell -File spikes/p4-probe/p4-probe.ps1
```

**Uyarı:** `p4-diff.ps1` ve `p4-watch.ps1` sentetik tuş gönderir (`keybd_event`).
Kabuk enjekte edilmiş girdiyi OSD yolunda eleyebilir — sonuç boş çıkarsa
`p4-probe.ps1`'i **fiziksel** ses tuşuna bastıktan hemen sonra çalıştır.

Sonuçlar ve yorumu: `docs/P4-ARASTIRMA.md`.
