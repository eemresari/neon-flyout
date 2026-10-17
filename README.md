# neon-flyout

> A replacement for the Windows media flyout, built with Tauri v2 and Rust: it reacts to playback state, not to key presses.

> 🇹🇷 **Türkçe özet:** neon-flyout, Windows'un kendi medya/ses kutusunun yerine geçen, Tauri v2 + Rust ile yazılmış bir masaüstü uygulamasıdır. Windows'un yalnızca tuşa tepki vermesinin aksine çalma durumunun kendisini (SMTC) izler; bu yüzden fare makrosu, uygulama penceresi veya kulaklıktan gelen komutlarda da görünür. Yığın kararı bir gecikme ölçümüyle verildi (p95 12,9 ms, bütçe 50 ms); P0'dan P4'e kadar aşamalar tamamlandı, paketleme (P5) uygulandı ve kurulum dosyası üretildi.

## Overview

When the track changes, playback starts or stops, or the volume is adjusted, a small box appears in a screen corner for about 2.5 seconds. The app reads the system media session (SMTC, `Windows.Media.Control`) and has no player of its own.

**How it differs from the built-in Windows flyout:** Windows shows its box when a media *key* is pressed. neon-flyout reacts to the *playback state itself*, so it also appears for commands that come from a mouse macro, an app window or a headset, which is exactly where Windows shows nothing. It also suppresses Windows' own flyout so the two never overlap.

There are two themes, `cyber` and `neon`. The full visual design (all eight states, both themes, on bright and dark backgrounds) is in [`design/preview.html`](design/preview.html); open it in a browser to see it at real size, rendered with the production CSS.

## Features

- Shows track title, artist, source app, playback state, position and cover art from the active media session
- Volume, mute and a live level meter (WASAPI peak) with a per-theme display: segmented meter in `cyber`, spectrum bars in `neon`
- Noise rules: ten key presses in a row produce a single box, a track change that emits eight events over ~800 ms opens once and updates in place, and an empty new session triggers nothing
- Suppresses the native Windows flyout and restores it on a clean exit (the detection knows the window classes of both current Windows 11 and older builds; it was measured on Windows 11)
- System tray menu: theme, screen corner, start with Windows, open the config file, reload, quit
- JSON config file for theme, corner, monitor (primary or cursor), display and fade times
- Per-user NSIS installer (no UAC prompt) with an embedded WebView2 bootstrapper

## Architecture

```text
SMTC / audio event -> adapter thread -> mpsc -> core + deadline -> emit -> WebView -> show window
```

The project follows a strict layering rule: each layer only knows the one below it.

| Layer | Responsibility |
|---|---|
| Presentation (WebView) | Rendering only: no logic, no timers. Receives a `PlayerState` snapshot. |
| IPC boundary | Events up (`app.emit`), commands down (`tauri::command`) |
| Core | Single source of truth (`PlayerState`), noise rules and the visibility state machine. Knows no Windows types, so it is the unit-testable layer. |
| Adapters | SMTC, audio, window, WebView, native-flyout suppressor, tray, autostart and config file. Each event source runs on its own thread; all `unsafe`, WinRT and COM code lives here. |

The window is created once at startup and never destroyed; showing and hiding is a plain `ShowWindow`. Details: [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) and the design-system rules in [docs/DESIGN.md](docs/DESIGN.md).

## Tech stack

| | |
|---|---|
| Shell | Tauri v2 (Rust, edition 2021) |
| Frontend | Vanilla TypeScript + Vite, plain CSS from `design/` |
| Windows APIs | `windows` crate: WinRT `Media.Control` (SMTC), Core Audio / WASAPI, user32, Shell links |
| WebView | WebView2 (via `webview2-com`) |
| Packaging | NSIS installer built by `tauri build` |
| Platform | Windows 11 x64 target; minimum Windows 10 1809 |

## Engineering highlights

- **Latency as an acceptance criterion.** The budget is 50 ms from trigger to first pixel. A dedicated measurement phase (P0) compared Tauri against a WPF window; Tauri's worst p95 was **12.9 ms** on the test machine, which decided the stack. The first measuring method (requestAnimationFrame) turned out to be invalid because the render loop does not run while the window is hidden, so it was replaced with a frame-independent desktop-pixel poll. Results: [docs/P0-SONUC.md](docs/P0-SONUC.md).
- **Idle cost.** Pausing the WebView2 controller while the box is hidden dropped frames produced while hidden from 9,235 to 0 in the measurement, with no measurable wake-up penalty ([docs/P3-SONUC.md](docs/P3-SONUC.md)).
- **SMTC event behavior was measured, not assumed.** A new session is born empty and filled ~14 ms later, and a single track change produced 8 events spread over 811 ms; the debounce window therefore controls "do not replay the animation", not "open the box" ([docs/P1-SONUC.md](docs/P1-SONUC.md)).
- **Native flyout suppression.** Detection uses the shell's window class pair, `explorer.exe` ownership and the undocumented window z-band; suppression is reversible and restored on exit. The write-up also records a measurement mistake (`EnumWindows` does not list windows in that band, so the first run wrongly concluded there was nothing to suppress) ([docs/P4-ARASTIRMA.md](docs/P4-ARASTIRMA.md)).
- **Tests.** The core layer is unit tested: `cargo test` reports 34 passing, 1 ignored (the autostart test writes to the real Startup folder) as of the P5 write-up. The tests encode the acceptance criteria themselves, for example "10 rapid triggers produce one box".

## Project status

| Phase | Scope | State |
|---|---|---|
| P0 | Latency gate, stack decision | Done: Tauri v2 chosen |
| P1 | SMTC spike | Done |
| P2 | Trigger rules and visibility timer | Done |
| P3 | UI wired to the design system | Done |
| P4 | Native flyout suppression | Done |
| P5 | Packaging: tray, config file, autostart, installer | Implemented, installer built (3.66 MB); the final check on a clean Windows install is still open |

Not done yet: a settings window, a project-specific icon, code signing (the installer is unsigned, so SmartScreen warns), auto-update. The phase documents are in Turkish and live in [docs/](docs/); the roadmap is [docs/ROADMAP.md](docs/ROADMAP.md).

## Repository layout

```text
app/
├── src/                  # Frontend (TypeScript): PlayerState -> DOM
└── src-tauri/src/
    ├── core/             # Config, engine, noise rules, visibility, placement (tested, no Windows types)
    └── adapters/         # smtc, audio, window, webview, native_flyout, tray, autostart, config_file
design/                   # Production CSS, themes, fonts, preview.html
docs/                     # Architecture, design system, roadmap, per-phase results
spikes/                   # Throwaway measurement code for P0, P1 and P4
```

## Running locally

Requires Windows, Node.js and a Rust toolchain (MSVC).

```bash
cd app
npm install
npm run tauri dev     # development (starts the Vite dev server itself)
npm run tauri build   # produces the NSIS installer
```

Running `cargo run` on its own does not work in debug builds because the WebView points at the Vite dev server.
