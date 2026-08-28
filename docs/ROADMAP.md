# Yol haritası

Her aşamanın bir çıkış kriteri var. Kriter sağlanmadan sonraki aşamaya geçme.

---

## P0 — Gecikme kapısı ✅ TAMAMLANDI (2026-08-28) — GEÇTİ

**Sonuç: Tauri p95 12.9 ms / bütçe 50 ms → yığın Tauri v2 + Rust.** Ölçüm yöntemi,
sayılar ve yan bulgular: **`docs/P0-SONUC.md`**. Aşağıdaki plan tarihsel kayıt olarak
duruyor; iki noktada uygulamadan ayrıldı:

- rAF ile ilk kareyi yakalama yöntemi **geçersiz çıktı** (render döngüsü pencere
  gizliyken durmuyor). Yerine masaüstü pikselini yoklayan, çerçeveden bağımsız yöntem
  kullanıldı.
- Yol haritasında olmayan iki profil eklendi: `hide-long` (60 sn derin bekleme) ve
  `offscreen-short` (alternatif gizleme). İkisi de karara katkı sağladı.

**Bu aşamayı tekrar çalıştırma; P1'den devam et.**

<details>
<summary>Özgün P0 planı</summary>

**Bu bir ölçüm, uygulama değil.** UI yok, tasarım yok, SMTC yok.

Yapılacak:

1. Boş bir Tauri v2 penceresi: `transparent: true`, `decorations: false`,
   `alwaysOnTop: true`, `skipTaskbar: true`, `focus: false`.
2. Pencereyi `SW_HIDE` ile gizle, bir tuşla `SW_SHOW` yap.
3. `QueryPerformanceCounter` ile **tetik anı → ilk boyanan kare** arasını ölç.
   İlk kareyi WebView tarafından `requestAnimationFrame` içinde bir IPC ile geri
   bildirerek yakala, ya da harici bir yüksek FPS kayıt ile doğrula.
4. En az 20 tekrar, p50 ve p95 raporla.
5. **Aynı testi bir WPF penceresiyle tekrarla** ve karşılaştır.

**Kapı:** Tauri p95 < 50 ms ise devam. Değilse plan aynı kalır, yığın WPF'e döner.
Sonucu `CLAUDE.md`'deki varsayım listesine yaz.

İkinci soru, aynı anda ölçülebilir: şeffaf pencerede `backdrop-filter: blur()`
Windows'ta çalışıyor mu?

</details>

---

## P1 — SMTC spike

Tauri yok, UI yok. Sadece bir Rust konsol uygulaması.

- Aktif oturumu bul, başlık / sanatçı / durum / pozisyon bas
- `CurrentSessionChanged` ile oturum değişimini izle
- `TryTogglePlayPauseAsync` ile bir komut gönder
- Paketlenmemiş exe'de `globalMediaControl` capability sorunu çıkıyor mu, not et

**Çıkış:** Spotify ↔ tarayıcı sekmesi geçişinde doğru metadata akıyor.

---

## P2 — Tetik + zamanlayıcı

Artık gerçek uygulama. İçerik hâlâ çirkin olabilir.

- Dört tetikleyiciyi bağla (`docs/ARCHITECTURE.md` → Tetikleyiciler)
- Gürültü kurallarının **dördünü de** yaz: ilk-olay yutma, no-op yutma,
  150 ms birleştirme, ön plan kuralı
- Tek deadline zamanlayıcısı + görünürlük durum makinesi
- Pencere gerçekten belirip kayboluyor, `SW_HIDE` ile

**Çıkış:** Ses tuşuna 10 kez arka arkaya bas — tek kutu görünüyor, üst üste binme yok,
titreme yok. Spotify penceresinden durdur — kutu çıkıyor (Windows'unki çıkmıyor).

---

## P3 — Arayüz

`design/` altındaki CSS'i frontend'e bağla.

- `flyout.css` + iki tema dosyası
- `PlayerState` → DOM
- Kapak görselini gizliyken önden çöz ve cache'le
- Giriş/çıkış animasyonu, 150 ms
- Tema değiştirme (`data-theme` attribute), köşe ve monitör seçimi, DPI

**Çıkış:** `design/preview.html`'deki sekiz durumun sekizi de uygulamada birebir aynı
görünüyor.

---

## P4 — Native flyout bastırma

Ayrı ve en sonda, çünkü uygulama bu olmadan da kullanılabilir olmalı.

- `NativeFlyoutSuppressor` modülü, sürüm başına strateji
- Bulunamazsa sessiz vazgeç, ayarlarda durumu göster

**Çıkış:** Bastırma başarısız olduğunda uygulama çalışmaya devam ediyor.

---

## P5 — Paketleme

- Tepsi ikonu, Windows ile başlat
- Ayar dosyası (tema, köşe, süre, monitör)
- WebView2 bootstrapper gömülü installer

**Çıkış:** Temiz bir Windows kurulumunda çift tıkla çalışıyor.

---

## Sonraya (v2)

- Uygulama bazlı ses kontrolü (`ISimpleAudioVolume`)
- Kutu üzerinde etkileşim (`WS_EX_TRANSPARENT` kaldırılıp tıklanabilir mod)
- Diğer temalar: pixel/bitmap, lo-fi pixel, hi-fi kaset dekk
- Uzun şarkı adı için kaydırmalı (marquee) başlık — şu an kesiliyor
