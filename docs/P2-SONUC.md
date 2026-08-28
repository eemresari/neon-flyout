# P2 — Tetik + zamanlayıcı sonucu

**Tarih:** 2026-08-28 · **Durum: kod tamam, birim testler geçti, manuel doğrulama kısmi.**

`spikes/` burada bitti. Uygulama artık **`app/`** altında yaşıyor ve
`docs/ARCHITECTURE.md`'deki katmanlara birebir oturuyor.

---

## 1. Ne kuruldu

`npm create tauri-app@latest` → vanilla + TypeScript, dizin `app/`, kimlik
`dev.neonflyout.app`. Pencere ayarları P0'da doğrulanan haliyle
(`transparent`, `decorations:false`, `alwaysOnTop`, `skipTaskbar`, `focus:false`,
`visible:false`, `resizable:false`, `shadow:false`).

```
app/
  index.html, src/main.ts, src/styles.css     P2 yer tutucusu — P3'te design/ ile değişecek
  src-tauri/src/
    main.rs        giriş
    lib.rs         pencereyi kurar, adapter thread'lerini başlatır, sürücü döngü
    core/
      model.rs     PlayerState, MediaSnapshot, VolumeSnapshot — Windows tipi YOK
      event.rs     DomainEvent: adapter → core tek arayüz
      rules.rs     dört gürültü kuralı + P1'in kesin maddeleri
      visibility.rs Hidden/Visible/FadeOut, tek deadline
      engine.rs    tek yazıcı; yan etki üretmez, Effect listesi döndürür
    adapters/
      smtc.rs      MTA, oturum başına sök-tak, durum tutmaz
      audio.rs     IAudioEndpointVolume + IMMNotificationClient
      window.rs    stil, konum, SW_HIDE, ön plan kontrolü
```

**Mimari kuralı tutuyor:** `unsafe`, WinRT tipi ve COM arayüzü yalnızca
`adapters/` içinde. `core/` Windows'a dair hiçbir tip bilmiyor ve zamanı bile
dışarıdan alıyor (`now_ms: u64`) — bu yüzden 2.5 saniyelik deadline'ı test etmek
için gerçekten beklemek gerekmiyor.

---

## 2. Core → WebView sözleşmesi — **P3'ün girdisi**

Tek kanal: `flyout`. Payload (`core::engine::Frame`, camelCase):

```jsonc
{
  "kind": "media" | "volume",
  "phase": "enter" | "update" | "leave",
  "sameBurst": true,          // bu olay bir öncekiyle aynı kullanıcı eyleminden mi
  "state": {
    "media": {
      "sourceId": "SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify",
      "track": { "title": "...", "artist": "...", "album": "" },
      "state": "playing" | "paused" | "stopped" | "changing" | "opened" | "closed" | "unknown",
      "position": { "positionMs": 132000, "durationMs": 212000, "ageMs": 2663 },  // veya null
      "controls": { "play": true, "pause": true, "toggle": true, "next": false,
                    "prev": false, "stop": true, "seek": true },
      "sourceIsForeground": false
    },
    "volume": { "level": 0.64, "muted": false, "deviceName": "Hoparlör (Realtek...)" }
  }
}
```

P3'ün DOM'a bağlarken bilmesi gerekenler:

- **`phase` animasyonu yönetir.** `enter` = giriş animasyonu oynasın; `update` =
  kutu zaten ekranda, **sadece içerik değişsin**; `leave` = çıkış animasyonu
  (150 ms), bitiminde pencere `SW_HIDE` ile gerçekten gizleniyor.
- **`update` fazı sık gelir.** Tek parça geçişi 811 ms'ye yayılan bir seri
  üretiyor ve başlık 270. ms'de geliyor (P1 § 6.3) — kutu ilk anlamlı olayda
  açılıyor, başlık sonradan **yerinde** yazılıyor. Bu geçişin göze batmaması
  P3'ün işi (`sameBurst` bunun için var).
- **`position: null` olabilir** — `LastUpdatedTime == 0` (P1 § 5.2).
  İnterpolasyona sokma.
- **`album` boş, `controls.next` false olabilir** (tarayıcı oturumları, P1 § 3).
- **Gizliyken hiçbir şey gönderilmiyor.** WebView boşuna uyandırılmıyor;
  `leave`'den sonra bir sonraki olay `enter` olur.

---

## 3. Gürültü kuralları — nereye yazıldı

Dördü de `core/rules.rs`'te, **P1'in düzelttiği hâliyle**:

| Kural | Uygulanışı |
|---|---|
| 1. İlk olayı yut | Tür başına (medya/ses) ilk olay durumu doldurur, kutuyu açmaz. Açılıştaki başlangıç anlık görüntüsünü bu yutuyor. |
| 2. Değişmeyeni yut | (başlık, sanatçı, durum) üçlüsü aynıysa at. Ses için (yüzde, mute) — float gürültüsü yuvarlanarak eleniyor. |
| 3. Birleştirme | Gösterim kararını **yönetmiyor**. İlk anlamlı olay kutuyu hemen açıyor; sonrakiler `update` fazıyla içeriği yerinde güncelleyip deadline'ı ileri alıyor. |
| 4. Ön plandakini atla | Yazıldı, **varsayılan kapalı** (`RuleConfig::skip_when_foreground`). Gerekçe § 5'te. |

P1'den gelen kesin maddeler de kodda:

- `CurrentSessionChanged` kutuyu açmıyor — **duruma bile dokunmuyor.** Yeni oturum
  boş doğduğu için kaynak adını hemen yazmak, içerik gelene kadar "yeni kaynak +
  eski parça" karması gösterirdi.
- `Opened` / `Closed` açmıyor; `Changing` tolere ediliyor.
- `TimelinePropertiesChanged`'e **hiç abone olunmuyor**. Pozisyon her anlık
  görüntüde taze okunuyor, ayrı olaya gerek yok.
- Ölmekte olan oturumdan okuma hatası sessizce atılıyor.

Görünürlük makinesi (`core/visibility.rs`) tek deadline tutuyor ve **ayrı bir
zamanlayıcı thread'i yok**: deadline'a kalan süre sürücü döngünün
`recv_timeout` beklemesi olarak geçiyor. Üst üste binen zamanlayıcı ihtimali
böylece yapısal olarak ortadan kalkıyor.

---

## 4. Doğrulama durumu

**Çıkış kriteri 5 — sağlandı.** `cargo test` → **16/16 geçti**. Testler
kriterlerin kendisini kodluyor:

| Test | Neyi doğruluyor |
|---|---|
| `on_ses_tusu_tek_kutu` | 10 art arda tetik → tek `SW_SHOW`, deadline ileri alınıyor (kriter 1) |
| `parca_gecisi_tek_kutu_acar_baslik_yerinde_guncellenir` | P1 § 6.3'teki seri → tek açılış + `update` (kriter 3) |
| `bos_dogan_oturum_kutuyu_acmaz_ve_icerigi_silmez` | Boş oturum hiçbir efekt üretmiyor, ekrandaki içeriği silmiyor (kriter 4) |
| `sure_dolunca_once_fade_sonra_gercek_gizleme` | `leave` → `SW_HIDE` sırası |
| `gizliyken_emit_yok` | Gizliyken WebView'e gönderim yok |
| + `rules.rs`'teki 6 kural testi | Dört gürültü kuralı ve P1 maddeleri |

**Kriter 1–4 (manuel) — kısmen doğrulandı.** Uygulama çalıştırıldı, kutu sağ üst
köşede beliriyor, kaynak ve çalma durumu doğru yazılıyor, süre dolunca
kayboluyor. **Ses tuşuna arka arkaya basma ve Spotify penceresinden fareyle
durdurma senaryoları ayrı ayrı teyit edilmedi** — P3 oturumunda ilk iş olarak
denenmeli, çünkü tasarım bağlandıktan sonra bir aksaklığın hangi katmandan
geldiğini ayırt etmek zorlaşır.

---

## 5. Kararlar ve sapmalar

- **`SW_SHOWNOACTIVATE`**, `SW_SHOW` yerine. `WS_EX_NOACTIVATE` zaten aktivasyonu
  engelliyor; bu ikinci kemer. Görünürlük mekanizması aynı, `SW_HIDE` kararı
  değişmedi.
- **Ön plan kuralı sezgisel, bu yüzden kapalı.** Paketli uygulamada AUMID birebir
  eşleşiyor (`GetApplicationUserModelId`), ama tarayıcılarda SMTC'nin verdiği
  kimlik bir kabuk AppID'si (`OperaSoftware.OperaGXWebBrowser.1783353490`), süreç
  AUMID'si değil — orada exe adının kimlikte geçip geçmediğine bakılıyor. Yanlış
  pozitifte kutu hiç çıkmaz; ayar açılmadan bu risk alınmıyor.
- **`windows-core = "0.61"` doğrudan bağımlılık.** `#[implement]` makrosu ürettiği
  kodda `windows_core::` yolunu kullanıyor; crate doğrudan bağımlılık olmadan
  derlenmiyor (`cannot find windows_core in the crate root`).

---

## 6. Tuzaklar — tekrar düşülmesin

**`cargo run` doğrudan çalıştırılırsa kutuda "localhost bağlantıyı reddetti"
çıkıyor.** Debug derlemesi `tauri.conf.json`'daki `devUrl`'e
(`http://localhost:1420`) gidiyor, `dist/`e değil. Doğru komut:

```bash
cd app && npm run tauri dev        # Vite'ı da başlatır
```

`dist/` üzerinden çalıştırmak isteniyorsa `npm run build` + release derlemesi.

**Smart App Control yine vurdu** — bu sefer `icu_properties_data`'nın build
script'ini engelledi (`os error 4551`), tekrar denemek çözmedi çünkü hash aynı
kalıyor. Çözüm `src-tauri/Cargo.toml`'a eklendi:

```toml
[profile.dev.build-override]
opt-level = 1
[profile.release.build-override]
opt-level = 1
```

Build script'ler farklı bayrakla derlenince hash değişiyor ve engel aşılıyor.
İşlevsel etkisi yok. **SAC kararı hâlâ açık** (kapatmak / imzalamak) — bu sadece
o kararı ertelemeye yarayan bir çare.

---

## 7. P3'e devredilenler

1. `design/flyout.css` + iki tema dosyasını frontend'e bağla; `app/src/styles.css`
   ve `index.html` şu an **yer tutucu**, oradaki değerleri `design/`'a taşıma —
   tersi geçerli.
2. Kapak görselini gizliyken önden çöz ve cache'le
   (`TryGetMediaPropertiesAsync().Thumbnail`, adapter'a eklenecek — P2'de hiç
   dokunulmadı).
3. `phase`/`sameBurst` üzerinden giriş/çıkış animasyonu, 150 ms.
4. Tema değiştirme (`data-theme`), köşe ve monitör seçimi, DPI. Şu an pencere
   **birincil monitörün** sağ üst köşesine, 16 px boşlukla konuluyor.
5. **WebView2'nin gizliyken render'ını durdur ve P0 ölçümünü tekrarla** —
   `CLAUDE.md`'deki 2. varsayım hâlâ açık ve asıl uyanma bedeli ancak gerçek UI
   varken ölçülebilir.
6. **AudioAdapter'ın cihaz değişimi yolu gerçek donanımla denenmedi**
   (`IMMNotificationClient` → yeniden bağlanma). Kulaklık takıp çıkararak
   doğrula; beklenmedik davranışta tahmin etme, küçük bir spike yaz.
