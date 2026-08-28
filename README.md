# neon-flyout

Windows medya flyout'u — Windows'un kendi kutusunun yerine geçer.

Parça değiştiğinde, çalma durumu değiştiğinde veya ses ayarlandığında ekranın sağ üst
köşesinde ~2.5 saniyelik bir kutu belirir. Sistem medya oturumunu (SMTC) okur, kendi
çalma motoru yoktur.

**Windows'unkinden farkı:** tuşa değil, çalma durumunun kendisine tepki verir. Bu yüzden
fare makrosundan, uygulama penceresinden veya kulaklıktan gelen komutlarda da görünür —
Windows'un hiçbir şey göstermediği durumlarda.

İki tema: `cyber` ve `neon`.

---

## Durum

Henüz kod yok. Tasarım ve plan hazır.

Başlamak için: **`CLAUDE.md`** oku, sonra `docs/ROADMAP.md` → P0.

P0 bir ölçüm ve sonucu yığın seçimini (Tauri v2 / .NET 8 WPF) belirliyor —
kod yazmadan önce o yapılmalı.

---

## Tasarıma bakmak için

`design/preview.html` dosyasını tarayıcıda aç. Sekiz durumun tamamı orada, gerçek
boyutta, gerçek CSS ile.

---

## Klasörler

```
CLAUDE.md      proje brifingi — buradan başla
docs/          mimari, yol haritası, tasarım sistemi
design/        üretim CSS'i + önizleme (mockup değil, gerçek kod)
```
