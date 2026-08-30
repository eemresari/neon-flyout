//! L3 — Core. **Windows'a dair hiçbir tip bilmez** (`docs/ARCHITECTURE.md`).
//!
//! Bu katmanda `unsafe` blok, WinRT tipi veya COM arayüzü yoktur; test edilebilir
//! olan tek katman budur ve P2'nin birim testleri buraya yazılır.
//!
//! Zaman da dışarıdan gelir: her fonksiyon `now_ms` (uygulama açılışından beri
//! geçen milisaniye) parametresi alır. Böylece 2.5 saniyelik deadline'ı test etmek
//! için gerçekten 2.5 saniye beklemek gerekmiyor.

pub mod config;
pub mod engine;
pub mod event;
pub mod model;
pub mod rules;
pub mod visibility;

pub use config::{Config, Corner, MonitorPick, Theme};
pub use engine::{Effect, Engine, EngineConfig};
pub use event::DomainEvent;
