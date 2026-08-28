// Release'te ek konsol penceresi acilmasin.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    neon_flyout_lib::run()
}
