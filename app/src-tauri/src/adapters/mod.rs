//! L2 — Adapters. **Windows API'lerinin yaşadığı tek katman.**
//!
//! `unsafe` blok, WinRT tipi ve COM arayüzü yalnızca burada görünür
//! (`docs/ARCHITECTURE.md` → "Mimari kuralı"). Her adapter kendi thread'inde,
//! kendi COM apartment'ında çalışır; dışarıya yalnızca `core::DomainEvent` verir.

pub mod audio;
pub mod native_flyout;
pub mod smtc;
pub mod webview;
pub mod window;
