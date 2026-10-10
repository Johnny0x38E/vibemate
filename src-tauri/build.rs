//! Prepare Tauri resources and configuration before compiling the app.

use std::env;
use std::path::PathBuf;

fn main() {
    tauri_build::build();
    embed_test_manifest_on_windows_msvc();
}

/// Embeds a Common Controls v6 manifest into the unit-test harness on Windows MSVC.
///
/// `tauri_build` attaches its manifest only to the app binary. Cargo builds the
/// test harness as a separate executable, so without this the loader can fall
/// back to comctl32 v5 and the test process fails at startup with
/// `STATUS_ENTRYPOINT_NOT_FOUND`. `rustc-link-arg-tests` applies only to test
/// targets, so the app binary is unaffected.
fn embed_test_manifest_on_windows_msvc() {
    println!("cargo:rerun-if-changed=windows-test.manifest");

    let is_windows_msvc = env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    if !is_windows_msvc {
        return;
    }

    let manifest_dir = PathBuf::from(
        env::var("CARGO_MANIFEST_DIR").expect("Cargo always sets CARGO_MANIFEST_DIR"),
    );
    let manifest = manifest_dir.join("windows-test.manifest");
    println!("cargo:rustc-link-arg-tests=/MANIFEST:EMBED");
    println!(
        "cargo:rustc-link-arg-tests=/MANIFESTINPUT:{}",
        manifest.display()
    );
}
