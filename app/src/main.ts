// Tasarım `design/` altında üretim kodu olarak duruyor ve buradan doğrudan
// import ediliyor — kopyalanmıyor (`docs/DESIGN.md`). Vite kökünün dışına
// çıkabilmesi için `vite.config.ts` → `server.fs.allow`.
import "../../design/fonts.css";
import "../../design/flyout.css";
import "../../design/themes/cyber.css";
import "../../design/themes/neon.css";
import "./styles.css";

import { listen } from "@tauri-apps/api/event";
import { invoke } from "@tauri-apps/api/core";

// --- core → WebView sözleşmesi (`docs/P2-SONUC.md` § 2) ---------------------

type Phase = "enter" | "update" | "leave";
type Kind = "media" | "volume";
type Theme = "cyber" | "neon";

interface Track {
  title: string;
  artist: string;
  /** Tarayıcı oturumlarında boş gelebilir. */
  album: string;
}

interface MediaSnapshot {
  sourceId: string;
  track: Track;
  state: "closed" | "opened" | "changing" | "stopped" | "paused" | "playing" | "unknown";
  /** `LastUpdatedTime == 0` olan oturumda **null** — interpolasyona sokma. */
  position: { positionMs: number; durationMs: number; ageMs: number } | null;
  /** Kapak görseli, data URL. Adapter gizliyken çözüp cache'liyor. */
  art: string | null;
}

interface VolumeSnapshot {
  level: number;
  muted: boolean;
  deviceName: string;
}

interface Frame {
  kind: Kind;
  phase: Phase;
  sameBurst: boolean;
  state: { media: MediaSnapshot | null; volume: VolumeSnapshot | null };
}

/** `peak` kanalı: WASAPI `IAudioMeterInformation::GetPeakValue`, 0–1. */
interface Peak {
  peak: number;
}

/** Ayarın WebView'i ilgilendiren kısmı — `lib.rs` → `UiConfig`. */
interface UiConfig {
  theme: Theme;
}

// --- sabitler ---------------------------------------------------------------

/** Backend `ayarlar` komutu yanıtlayana kadarki ilk boyama. Canlı deneme: `__tema()`. */
const VARSAYILAN_TEMA: Theme = "cyber";

/** `design/flyout.css` → `.fly__meter` 16 segment, `.fly__spectrum` 24 çubuk. */
const METRE_SEGMENT = 16;
const SPEKTRUM_CUBUK = 24;
/** `docs/DESIGN.md`: on = round(level / 100 * 24). */
const TICK_SAYISI = 24;

/** `design/preview.html`'deki referans uygulamadan birebir. */
const GLYPH = {
  play: '<svg width="11" height="13" viewBox="0 0 11 13" aria-hidden="true"><path d="M0 .5L11 6.5 0 12.5V.5Z" fill="currentColor"/></svg>',
  pause: '<svg width="11" height="13" viewBox="0 0 11 13" aria-hidden="true"><rect x=".5" y=".5" width="3.6" height="12" fill="currentColor"/><rect x="7" y=".5" width="3.6" height="12" fill="currentColor"/></svg>',
  vol: '<svg width="26" height="22" viewBox="0 0 26 22" fill="none" aria-hidden="true"><path d="M2 8h4.5L12 3.5v15L6.5 14H2V8Z" fill="rgba(0,0,0,.5)" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"/><path d="M16 8c1.3 1 1.3 5 0 6" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/><path d="M19.5 5c3 2.3 3 9.7 0 12" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/></svg>',
  mute: '<svg width="26" height="22" viewBox="0 0 26 22" fill="none" aria-hidden="true"><path d="M2 8h4.5L12 3.5v15L6.5 14H2V8Z" fill="rgba(0,0,0,.5)" stroke="currentColor" stroke-width="1.6" stroke-linejoin="round"/><path d="m16.5 8 6.5 6M23 8l-6.5 6" stroke="currentColor" stroke-width="1.8" stroke-linecap="round"/></svg>',
};

const DURUM: Record<MediaSnapshot["state"], string> = {
  playing: "Çalıyor",
  paused: "Duraklatıldı",
  stopped: "Durduruldu",
  changing: "Değişiyor",
  opened: "Açıldı",
  closed: "Kapandı",
  unknown: "—",
};

// --- DOM --------------------------------------------------------------------

const el = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;

const kutuMedya = el("media");
const kutuSes = el("volume");

const mArt = el<HTMLImageElement>("m-art");
const mGlyph = el("m-glyph");
const mPct = el("m-pct");
const mState = el("m-state");
const mSource = el("m-source");
const mTitle = el("m-title");
const mArtist = el("m-artist");
const mElapsed = el("m-elapsed");
const mTotal = el("m-total");
const mSpectrum = el("m-spectrum");
const mMeter = el("m-meter");

const vIcon = el("v-icon");
const vState = el("v-state");
const vSource = el("v-source");
const vTicks = el("v-ticks");
const vPct = el("v-pct");

/** Sabit sayıda çocuk: bir kez kurulur, sonra yalnızca sınıf/değişken yazılır. */
function doldur(host: HTMLElement, n: number): HTMLElement[] {
  const kids: HTMLElement[] = [];
  for (let i = 0; i < n; i++) {
    const bar = document.createElement("i");
    host.appendChild(bar);
    kids.push(bar);
  }
  return kids;
}

const segmentler = doldur(mMeter, METRE_SEGMENT);
const cubuklar = doldur(mSpectrum, SPEKTRUM_CUBUK);
const tickler = doldur(vTicks, TICK_SAYISI);

// --- durum ------------------------------------------------------------------

let tema: Theme = VARSAYILAN_TEMA;
/** Ekranda olan (ya da en son olan) kutu. */
let ekranda: HTMLElement | null = null;
/** `img.src`'yi boşuna yeniden yazmamak için — yeniden yazmak kapağı kırpıştırıyor. */
let sonArt: string | null = null;
let sonMute = false;
/** Spektrumun geçmişi: en yenisi sağda. */
const gecmis = new Array<number>(SPEKTRUM_CUBUK).fill(0);

// --- yardımcılar ------------------------------------------------------------

/** AUMID uzun ve çirkin: "SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify" → "Spotify". */
function kaynakAdi(id: string): string {
  const son = id.split("!").pop() ?? id;
  return son.split(".").pop() ?? son;
}

function sure(ms: number): string {
  const t = Math.max(0, Math.round(ms / 1000));
  return `${Math.floor(t / 60)}:${String(t % 60).padStart(2, "0")}`;
}

const kis = (x: number, a: number, b: number) => Math.min(Math.max(x, a), b);

function sinif(node: HTMLElement, want: string) {
  if (node.className !== want) node.className = want;
}

// --- tema -------------------------------------------------------------------

function temaUygula(t: Theme) {
  tema = t;
  kutuMedya.dataset.theme = t;
  kutuSes.dataset.theme = t;
  sesIkonu(sonMute);
}

/** Ses ikonu tek yapısal tema farkı: neon onu bir halkanın içine alıyor. */
function sesIkonu(muted: boolean) {
  const g = muted ? GLYPH.mute : GLYPH.vol;
  vIcon.innerHTML =
    tema === "neon"
      ? `<div class="fly__ring"><span class="fly__glyph">${g}</span></div>`
      : g;
}

// --- render -----------------------------------------------------------------

function medyaYaz(m: MediaSnapshot | null) {
  if (!m) return;

  // `data-state` yalnızca dört değer alır (`docs/DESIGN.md`); metin daha ayrıntılı.
  kutuMedya.dataset.state = m.state === "playing" ? "playing" : "paused";
  mState.textContent = DURUM[m.state] ?? "—";
  mSource.textContent = kaynakAdi(m.sourceId);
  mTitle.textContent = m.track.title || "(başlık yok)";
  mArtist.textContent = m.track.artist;
  mGlyph.innerHTML = m.state === "playing" ? GLYPH.play : GLYPH.pause;

  let yuzde = 0;
  let gecen = "--:--";
  let toplam = "--:--";
  const p = m.position;
  if (p && p.durationMs > 0) {
    // Ölçüm `ageMs` kadar bayat. Çalarken o kadar ilerlemiş sayılır, duraklatılmışken
    // ilerlemez. Sürekli interpolasyon YOK — kutu 2.5 sn duruyor, tek okuma yeter.
    const pos = kis(p.positionMs + (m.state === "playing" ? p.ageMs : 0), 0, p.durationMs);
    yuzde = Math.round((pos / p.durationMs) * 100);
    gecen = sure(pos);
    toplam = sure(p.durationMs);
  }
  kutuMedya.style.setProperty("--progress", String(yuzde));
  mElapsed.textContent = gecen;
  mTotal.textContent = toplam;
  mPct.textContent = `${yuzde}%`;

  if (m.art !== sonArt) {
    sonArt = m.art;
    if (m.art) {
      mArt.src = m.art;
      mArt.hidden = false;
    } else {
      mArt.removeAttribute("src");
      mArt.hidden = true; // kapak yoksa `--art-fallback` gradyanı görünsün
    }
  }
}

function sesYaz(v: VolumeSnapshot | null) {
  if (!v) return;

  const yuzde = Math.round(kis(v.level, 0, 1) * 100);
  kutuSes.dataset.state = v.muted ? "mute" : "volume";
  kutuSes.style.setProperty("--progress", String(v.muted ? 0 : yuzde));
  vState.textContent = v.muted ? "Sessiz" : "Ses Seviyesi";
  vSource.textContent = v.deviceName;
  vPct.textContent = String(yuzde);

  if (v.muted !== sonMute) {
    sonMute = v.muted;
    sesIkonu(v.muted);
  }

  const acik = v.muted ? 0 : Math.round((yuzde / 100) * TICK_SAYISI);
  for (let i = 0; i < TICK_SAYISI; i++) {
    sinif(tickler[i], i < acik ? (i >= acik - 3 ? "is-hi" : "is-on") : "");
  }
}

function render(f: Frame) {
  const kutu = f.kind === "media" ? kutuMedya : kutuSes;
  if (f.kind === "media") medyaYaz(f.state.media);
  else sesYaz(f.state.volume);

  if (f.phase === "leave") {
    // Çıkış animasyonu. Pencereyi 150 ms sonra core gerçekten `SW_HIDE` ediyor;
    // eleman DOM'da kalır, bir sonraki `enter` onu yeniden görünür yapar.
    kutu.classList.remove("is-visible");
    return;
  }

  // Tür değiştiyse (medya ↔ ses) eskisini animasyonsuz indir: pencere zaten
  // ekranda, iki kutunun üst üste binmesi titreme olur.
  if (ekranda && ekranda !== kutu) {
    ekranda.classList.remove("is-visible");
    ekranda.hidden = true;
  }

  const yeniAcilis = ekranda !== kutu;

  if (kutu.hidden) {
    kutu.hidden = false;
    // Reflow zorlanmazsa tarayıcı `hidden` kalkmasıyla `.is-visible` eklenmesini
    // tek karede birleştirir ve giriş animasyonu hiç oynamaz.
    void kutu.offsetWidth;
  }

  // Yeni açılışta metre geçmişi bayat; sıfırdan doldurulsun.
  if (yeniAcilis) {
    gecmis.fill(0);
    tepeYaz(0);
  }

  kutu.classList.add("is-visible");
  ekranda = kutu;
}

/**
 * WASAPI tepe değeri → iki tema iki farklı gösterge kullanıyor:
 * cyber dikey segment metre (anlık), neon yatay spektrum (son 24 örneğin
 * geçmişi, en yenisi sağda).
 */
function tepeYaz(peak: number) {
  const p = kis(peak, 0, 1);

  const acik = Math.round(p * METRE_SEGMENT);
  for (let i = 0; i < METRE_SEGMENT; i++) {
    sinif(segmentler[i], i < acik ? "is-on" : i === acik ? "is-hot" : "");
  }

  gecmis.shift();
  gecmis.push(Math.round(p * 100));
  const tepe = Math.max(...gecmis);
  for (let i = 0; i < SPEKTRUM_CUBUK; i++) {
    cubuklar[i].style.setProperty("--h", String(gecmis[i]));
    sinif(cubuklar[i], tepe > 0 && gecmis[i] === tepe ? "is-peak" : "");
  }
}

// --- bağlan -----------------------------------------------------------------

// Temayı önce senkron uygula: WebView backend'den önce boyayabilir, kutu temasız
// çıkmasın. Ayar gelince üstüne yazılır.
temaUygula(VARSAYILAN_TEMA);

// Emit beklenmiyor — WebView emit'ten sonra yüklenirse temayı kaçırırdı (P3'te
// `Frame` için aynısı yaşandı). Init'te ayarı kendimiz çekiyoruz.
invoke<UiConfig>("ayarlar")
  .then((c) => temaUygula(c.theme))
  .catch(() => {});

listen<UiConfig>("config", (e) => temaUygula(e.payload.theme));
listen<Frame>("flyout", (e) => render(e.payload));
listen<Peak>("peak", (e) => tepeYaz(e.payload.peak));

// Sekiz durumu `design/preview.html` ile karşılaştırmak için: devtools'ta ya da
// tarayıcıda `__tema("neon")` / `__kare({...})`. Uygulamanın davranışını
// değiştirmiyor, yalnızca render yolunu dışarıdan çağrılabilir yapıyor.
Object.assign(window, { __tema: temaUygula, __kare: render, __tepe: tepeYaz });
