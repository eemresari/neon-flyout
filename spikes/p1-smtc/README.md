# P1 — SMTC spike

**Atılacak kod.** Uygulamanın iskeleti değil; `docs/ROADMAP.md` → P1'in cevaplarını
almak için yazıldı. Sonuçlar: `docs/P1-SONUC.md`.

```bash
cd spikes/p1-smtc
cargo build --release
./target/release/p1-smtc.exe
```

Konsol açıldığında aktif oturumun anlık görüntüsü basılır, sonra canlı olay akışı
akmaya başlar. Komutlar (satır sonu ile):

| Tuş | Ne yapar |
|---|---|
| `enter` | Aktif oturumun anlık görüntüsü (metadata dâhil) |
| `t` | `TryTogglePlayPauseAsync` |
| `n` / `p` | `TrySkipNextAsync` / `TrySkipPreviousAsync` |
| `l` | Tüm SMTC oturumlarını listele |
| `q` | Çık (abonelikleri söker, olay sayaçlarını basar) |

Betikle sürmek için stdin'e yaz:

```bash
(sleep 2; echo t; sleep 4; echo t; sleep 1; echo q) | ./target/release/p1-smtc.exe | tee run.log
```

Her satır başında uygulama başlangıcından itibaren geçen süre var — parça geçişinin
kaç olay ürettiğini ve olayların ne kadar aralıkla geldiğini oradan okuyabilirsin.
P2'nin gürültü kuralları bu kayda dayanıyor.
