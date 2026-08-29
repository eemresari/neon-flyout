# P4 ölçüm düzeneği — native flyout penceresini ara

Salt-okunur. Hiçbir pencereyi gizlemez, taşımaz, değiştirmez. Rust gerektirmez —
Smart App Control `rustc`'yi engellediği için bilerek PowerShell (bkz. `CLAUDE.md`).

| Betik | Ne yapar |
|---|---|
| `p4-probe.ps1` | `XamlExplorerHostIslandWindow` ve `NativeHWNDHost` örneklerini listeler: sahip süreç, z-band, alt pencereler, rect |
| `p4-diff.ps1` | `VOLUME_UP` gönderir, 350 ms sonra **görünür hâle gelen** pencereleri yazar, sesi geri alır |
| `p4-watch.ps1` | `VOLUME_UP` gönderir, 2 sn boyunca 60 ms'de bir **tüm** üst düzey pencereleri örnekler; yeni pencere / görünürlük / konum değişimlerini raporlar |

```powershell
powershell -File spikes/p4-probe/p4-probe.ps1
```

**Uyarı:** `p4-diff.ps1` ve `p4-watch.ps1` sentetik tuş gönderir (`keybd_event`).
Kabuk enjekte edilmiş girdiyi OSD yolunda eleyebilir — sonuç boş çıkarsa
`p4-probe.ps1`'i **fiziksel** ses tuşuna bastıktan hemen sonra çalıştır.

Sonuçlar ve yorumu: `docs/P4-ARASTIRMA.md`.
