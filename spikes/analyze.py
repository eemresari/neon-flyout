#!/usr/bin/env python3
"""P0 ölçüm CSV'lerini özetler.

Kullanım:  python spikes/analyze.py

measurements/*.csv okur ve markdown tablosu basar. Bağımlılık yok, saf stdlib.

ASIL ÖLÇÜT `screen_ms`: masaüstü pikselinin değiştiği an — yani DWM'in gerçekten
sunduğu kare. `raf1`/`raf2` (çerçevenin kendi render callback'i) sadece teşhis
içindir; her iki çerçevede de bu döngü pencere gizliyken dönmeye devam ettiği
için kapı kararında KULLANILMAZ.

Isınma tekrarları CSV'ye zaten yazılmıyor (spike sürücüleri atıyor), burada
ayrıca atma yapılmaz — dosyadaki her satır gerçek ölçümdür.
"""

import csv
import math
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MEAS = ROOT / "measurements"

# Kapı: ROADMAP P0 → tetikten ilk piksele 50 ms bütçesi.
GATE_MS = 50.0
GRAY_LO, GRAY_HI = 40.0, 60.0


def percentile(values, p):
    """Doğrusal interpolasyonlu yüzdelik (numpy'siz)."""
    if not values:
        return float("nan")
    s = sorted(values)
    if len(s) == 1:
        return s[0]
    k = (len(s) - 1) * p
    lo, hi = int(k), min(int(k) + 1, len(s) - 1)
    return s[lo] + (s[hi] - s[lo]) * (k - lo)


def clean(xs):
    return [x for x in xs if not math.isnan(x)]


def fnum(x, nd=1):
    return "—" if math.isnan(x) else f"{x:.{nd}f}"


def summarize(path):
    screen, raf1, raf2, idle, vis = [], [], [], [], []
    with path.open(newline="", encoding="utf-8") as fh:
        for row in csv.DictReader(fh):
            screen.append(float(row.get("screen_ms", "nan") or "nan"))
            raf1.append(float(row.get("raf1_ms", "nan") or "nan"))
            raf2.append(float(row.get("raf2_ms", "nan") or "nan"))
            idle.append(int(row.get("idle_ticks", 0) or 0))
            vis.append(int(row.get("visible_while_idle", 0) or 0))
    return {
        "name": path.stem,
        "n": len(raf1),
        "screen": clean(screen),
        "raf1": clean(raf1),
        "raf2": clean(raf2),
        "idle_max": max(idle) if idle else 0,
        "idle_sum": sum(idle),
        "vis_any": any(v == 1 for v in vis),
        "has_screen": bool(clean(screen)),
    }


def main():
    files = sorted(MEAS.glob("*.csv"))
    if not files:
        print(f"CSV bulunamadi: {MEAS}", file=sys.stderr)
        return 1

    stats = [summarize(f) for f in files]

    print("### Asil olcut — ekran pikseli (tetik -> DWM'in sundugu kare)")
    print()
    print("| Kosu | n | p50 | **p95** | max |")
    print("|---|---:|---:|---:|---:|")
    for s in stats:
        if not s["has_screen"]:
            print(f"| `{s['name']}` | {s['n']} | — | **—** | — |")
            continue
        print(
            f"| `{s['name']}` | {len(s['screen'])} "
            f"| {fnum(percentile(s['screen'], .50))} "
            f"| **{fnum(percentile(s['screen'], .95))}** "
            f"| {fnum(max(s['screen']))} |"
        )
    print()
    print("Sayilar ms.")
    print()

    print("### Teshis — cerceve render callback'i (KARAR ICIN DEGIL)")
    print()
    print("| Kosu | raf1 p50 | raf2 p50 | raf2 p95 | idle_ticks (max) | gizliyken Win32'ye gore gorunur |")
    print("|---|---:|---:|---:|---:|:--|")
    for s in stats:
        print(
            f"| `{s['name']}` "
            f"| {fnum(percentile(s['raf1'], .50), 2)} "
            f"| {fnum(percentile(s['raf2'], .50), 2)} "
            f"| {fnum(percentile(s['raf2'], .95), 2)} "
            f"| {s['idle_max']} | {'EVET' if s['vis_any'] else 'hayir'} |"
        )
    print()

    print("### Saglik kontrolu")
    for s in stats:
        if s["idle_sum"] > 2:
            note = (f"render dongusu gizliyken de doniyor ({s['idle_sum']} kare) "
                    f"-> raf sutunlari uyanma olcumu DEGIL, screen_ms kullan")
        else:
            note = "render dongusu gizliyken duruyor"
        print(f"- `{s['name']}`: {note}")
        if not s["has_screen"]:
            print(f"  - **screen_ms yok** — bu dosya eski/gecersiz bir kosudan, karara dahil etme.")
    print()

    print("### Kapi")
    gate = [s for s in stats if s["name"].startswith("tauri-hide") and s["has_screen"]]
    if not gate:
        print("- Gecerli Tauri hide-* kosusu yok -> **kapi degerlendirilemedi.**")
        wpf = [s for s in stats if s["name"].startswith("wpf-hide") and s["has_screen"]]
        if wpf:
            worst = max(percentile(s["screen"], .95) for s in wpf)
            print(f"- Referans olarak WPF hide-* en kotu p95 = **{worst:.1f} ms** (butce {GATE_MS:.0f} ms).")
        return 0

    worst = max(percentile(s["screen"], .95) for s in gate)
    names = ", ".join(f"`{s['name']}`" for s in gate)
    print(f"- Belirleyici kosular: {names}")
    print(f"- En kotu screen p95 = **{worst:.1f} ms** (butce {GATE_MS:.0f} ms)")
    if GRAY_LO <= worst <= GRAY_HI:
        print(f"- **GRI BANT ({GRAY_LO:.0f}-{GRAY_HI:.0f} ms).** Harici yuksek FPS kayit ile dogrulanmali.")
    elif worst < GATE_MS:
        print("- **GECTI** -> yigin Tauri v2 + Rust olarak kalir.")
    else:
        print("- **KALDI** -> yigin .NET 8 + WPF'e doner.")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
