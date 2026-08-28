# Tasarım sistemi

İki tema: **cyber** ve **neon**. İkisi de aynı DOM'u kullanır; fark tamamen CSS'te.
Tema `data-theme` attribute'u ile değişir, tek satır.

`design/preview.html`'i tarayıcıda aç — sekiz durumun tamamı orada.

---

## DOM sözleşmesi

İki varyant var. Elemanlar **her zaman DOM'da bulunur**; tema kullanmadığını
`display: none` ile gizler. Bu sayede tema değişimi tek attribute yazmaktan ibaret.

### Medya kutusu — 400×120

```html
<div class="fly fly--media" data-theme="cyber" data-state="playing">
  <div class="fly__plate">
    <div class="fly__art"><img alt=""></div>
    <div class="fly__ring" style="--progress: 46"></div>
    <div class="fly__body">
      <div class="fly__head">
        <span class="fly__dot"></span>
        <span class="fly__state">Çalıyor</span>
        <span class="fly__source">Spotify</span>
      </div>
      <div class="fly__title">Midnight Protocol</div>
      <div class="fly__artist">Neon Division</div>
      <div class="fly__progress" style="--progress: 46">
        <span class="fly__time">1:47</span>
        <div class="fly__track"><div class="fly__fill"></div><div class="fly__head-tip"></div></div>
        <span class="fly__time">3:52</span>
      </div>
      <div class="fly__spectrum"><i style="--h: 5"></i> … 24 adet … </div>
    </div>
    <div class="fly__meter"><i></i> … 16 adet … </div>
    <div class="fly__deco"></div>
  </div>
</div>
```

### Ses kutusu — 400×84

```html
<div class="fly fly--volume" data-theme="cyber" data-state="volume">
  <div class="fly__plate">
    <div class="fly__icon"><svg>…</svg></div>
    <div class="fly__body">
      <div class="fly__head">
        <span class="fly__state">Ses Seviyesi</span>
        <span class="fly__source">Realtek · Hoparlör</span>
      </div>
      <div class="fly__ticks"><i class="is-on"></i> … 24 adet … </div>
    </div>
    <div class="fly__num">64<sup>%</sup></div>
  </div>
</div>
```

### Durumlar

`data-state` = `playing` | `paused` | `volume` | `mute`

Renkler, ikonlar ve metre davranışı buradan türer. JS'in yapacağı tek şey bu
attribute'u ve içerik metinlerini yazmak.

### Dinamik değerler

CSS custom property olarak geçirilir, sınıf hesaplamaya gerek yok:

| Değişken | Nerede | Değer |
|---|---|---|
| `--progress` | `.fly__ring`, `.fly__progress` | 0–100 |
| `--h` | `.fly__spectrum > i` | 0–100, peak metre çıktısı |
| `--level` | `.fly__ticks` | 0–100, ses seviyesi |

`.fly__ticks > i.is-on` ve `.is-hi` sınıfları seviyeye göre JS tarafından atanır
(24 tick, `Math.round(level / 100 * 24)`).

---

## Tema token'ları

Semantik isimler kullanılıyor — `--accent`, `--plate` — literal renk adları değil.
Böylece üçüncü bir tema eklendiğinde CSS'te hiçbir şey değişmiyor.

| Token | cyber | neon | Rol |
|---|---|---|---|
| `--accent` | `#4FE9F7` *(cyber-turkuaz)* | `#34E27A` *(neon-yeşil)* | Ana durum rengi |
| `--accent-2` | `#2AA9BC` | `#2FD8F5` *(neon-turkuaz)* | Yapı, ikincil |
| `--alert` | `#F0B03D` *(cyber-sarı)* | `#E93DBE` *(neon-magenta)* | Duraklatıldı, sessiz |
| `--alert-2` | `#C08A28` | `#A85CF5` *(neon-mor)* | Alert geçişinin ucu |
| `--spark` | `#F0B03D` | `#F5D43D` *(neon-sarı)* | Tek vurgu detayı, tepe |
| `--plate` | `#071016` | `#101B31` | Opak taban — kontrastı bu taşıyor |
| `--plate-2` | `#04090C` | `#0A1120` | Taban gradyanının ucu |
| `--ink` | `#E8F8FB` | `#EAF2FF` | Şarkı adı |
| `--ink-dim` | `#7DA3AE` | `#8E7BD6` | Sanatçı, süre |
| `--ink-faint` | `#55818C` | `#4E6A93` | Süre, yüzde |
| `--font-display` | Chakra Petch 600 | Rajdhani 700 | Başlık |
| `--font-mono` | JetBrains Mono | Space Mono | Etiket, veri |

Fontlar Google Fonts'tan geliyor. Uygulama **offline çalışmalı** — P3'te fontları
`.woff2` olarak indirip `@font-face` ile yerelden yükle, CDN linkini bırakma.

---

## Temaların yapısal farkları

Neon, cyber'ın renk değişmiş hali **değil**. CSS'te override edilen yapısal noktalar:

| | cyber | neon |
|---|---|---|
| Köşe | Diyagonal kesik (`clip-path`, açılı) | Basamaklı, dik açılı L çentik |
| Kenarlık | 1px | 2px |
| Kompozisyon | `art · body · meter` | `ring · body · art` *(aynalanmış)* |
| İlerleme | Yatay çubuk `.fly__progress` | Dairesel halka `.fly__ring` (`conic-gradient`) |
| Seviye | Dikey segment metre `.fly__meter` | Yatay spektrum `.fly__spectrum` |
| Taban | Neredeyse siyah | Lacivert + ince ızgara dokusu |
| Hue sayısı | 1 + durum rengi | 4 |
| Duraklatıldı | Sarı | Magenta |

Kompozisyon aynalaması `grid-template-areas` ile yapılıyor, DOM sırası değişmiyor.

---

## Değişmez ölçüler

Tema ne olursa olsun sabit:

- Medya kutusu **400×120**, ses kutusu **400×84**
- Ekranın sağ üst köşesinden 18px içeride (ayarlanabilir)
- Giriş 150 ms, bekleme 2500 ms, çıkış 150 ms
- Şarkı adı tek satır, taşarsa `text-overflow: ellipsis`

### Doğrulanmış ölçüm (headless Chromium, 2026-08)

`preview.html` render edildi; iki temada da kutu tam **400×120**, hiçbir eleman
plakadan taşmıyor. Tek uyarı: **başlık alanı dar.** Gerçek fontlar yüklü değilken
(yedek fontla) cyber tam sınırda oturuyor (237/237 px), neon 6 px taşıp kesiliyor
(216/210 px). Chakra Petch ve Rajdhani ikisi de dar (condensed) fontlar, yüklendiğinde
rahat sığıyorlar — ama pay çok az.

Bunun iki sonucu var:

1. **P3'te fontlar yerelden yüklenmeli ve başlık genişliği gerçek fontla tekrar
   ölçülmeli.** CDN'e bağımlı kalırsan internet yokken başlık kesilir.
2. Uzun şarkı adları zaten kesilecek. Kaydırmalı (marquee) başlık v2'ye bırakıldı;
   `…` ile kesmek kabul edilen davranış.

---

## Kontrast kuralı — pazarlık yok

Kutu **rastgele bir arka planın üstünde** okunabilmek zorunda: parlak oyun sahnesi,
karanlık oyun sahnesi, beyaz tarayıcı. Bu yüzden:

- Taban plakası **opak** (≥ 0.96 alfa). Şeffaflığı artırma.
- Parlaklık (`drop-shadow`) dekoratiftir, kontrastı o taşımaz — plaka taşır.
- Neon teması parlak zeminde cyber'dan zayıf; yeşil ve sarı en çok bedel ödeyen
  hue'lar. Kullanıcıya not düşülebilir ama tasarım değiştirilmemeli.
