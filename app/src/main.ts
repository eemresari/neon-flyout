import { listen } from "@tauri-apps/api/event";

// P2 arayüzü bilerek çirkin: bu aşamada doğrulanan şey görünüş değil, kutunun
// doğru anda belirip doğru anda kaybolması. Tasarım P3'te `design/` altından
// bağlanacak — buradaki markup ona göre yeniden yazılacak.

type Phase = "enter" | "update" | "leave";
type Kind = "media" | "volume";

interface Track {
  title: string;
  artist: string;
  album: string;
}

interface MediaSnapshot {
  sourceId: string;
  track: Track;
  state: "closed" | "opened" | "changing" | "stopped" | "paused" | "playing" | "unknown";
  position: { positionMs: number; durationMs: number; ageMs: number } | null;
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

const fly = document.getElementById("fly")!;
const elState = document.getElementById("state")!;
const elSource = document.getElementById("source")!;
const elTitle = document.getElementById("title")!;
const elSub = document.getElementById("sub")!;

const DURUM: Record<string, string> = {
  playing: "Çalıyor",
  paused: "Duraklatıldı",
  stopped: "Durduruldu",
  changing: "Değişiyor",
  opened: "Açıldı",
  closed: "Kapandı",
  unknown: "—",
};

/// AUMID uzun ve çirkin: "SpotifyAB.SpotifyMusic_zpdnekdrzrea0!Spotify" → "Spotify".
function kaynakAdi(id: string): string {
  const son = id.split("!").pop() ?? id;
  return son.split(".").pop() ?? son;
}

function render(frame: Frame) {
  fly.dataset.kind = frame.kind;
  fly.dataset.phase = frame.phase;

  if (frame.kind === "volume") {
    const v = frame.state.volume;
    if (!v) return;
    const yuzde = Math.round(v.level * 100);
    fly.dataset.state = v.muted ? "mute" : "volume";
    elState.textContent = v.muted ? "Sessiz" : "Ses Seviyesi";
    elSource.textContent = v.deviceName;
    elTitle.textContent = `${yuzde}%`;
    elSub.textContent = "";
    return;
  }

  const m = frame.state.media;
  if (!m) return;
  fly.dataset.state = m.state;
  elState.textContent = DURUM[m.state] ?? m.state;
  elSource.textContent = kaynakAdi(m.sourceId);
  elTitle.textContent = m.track.title || "(başlık yok)";
  elSub.textContent = m.track.artist;
}

listen<Frame>("flyout", (e) => render(e.payload));
