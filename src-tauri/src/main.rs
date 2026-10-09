//! Executable entry point; runtime setup lives in the library.

// A release build should open the desktop window without a Windows console.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    vibemate_lib::run();
}
