//! Prepare Tauri resources and configuration before compiling the app.

use std::env;

fn main() {
    tauri_build::build();
    declare_common_controls_v6_on_windows_msvc();
}

/// Declares a Common Controls v6 dependency for every linked Windows MSVC target.
///
/// `tauri_build` attaches its manifest only to the app binary. Cargo builds the
/// unit-test harness as a separate executable, so without this the loader can
/// fall back to comctl32 v5 and the test process fails at startup with
/// `STATUS_ENTRYPOINT_NOT_FOUND`. `/MANIFESTDEPENDENCY` adds the dependency to
/// the generated manifest, so it works for the test harness and is harmless for
/// the app binary, which already declares the same dependency.
///
/// `cargo:rustc-link-arg=` is used instead of `rustc-link-arg-tests`, because
/// Cargo rejects the test-specific form for packages without a `[[test]]` target.
fn declare_common_controls_v6_on_windows_msvc() {
    let is_windows_msvc = env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows")
        && env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    if !is_windows_msvc {
        return;
    }

    println!(
        "cargo:rustc-link-arg=/MANIFESTDEPENDENCY:type='win32' name='Microsoft.Windows.Common-Controls' version='6.0.0.0' processorArchitecture='*' publicKeyToken='6595b64144ccf1df' language='*'"
    );
}
