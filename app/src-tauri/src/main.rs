// Release'te ek konsol penceresi acilmasin.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Kurulumun tamami `lib.rs`'te; burasi yalnizca giris noktasi.
fn main() {
    neon_flyout_lib::run()
}
