//! The Windows executable's stack: Windows gives the main thread 1 MiB, and the game overflows it
//! right after the loading screen. Horizon's `horizon-client` reserves 8 MiB in its build script
//! (`packaging/windows/exe.rs`), which this package doesn't run; this reserves 64 MiB, what the
//! Windows builds were tested with (horizon/dev/DIST.md). Nothing for other targets.
const STACK: u32 = 64 * 1024 * 1024;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let arg = match std::env::var("CARGO_CFG_TARGET_ENV").as_deref() {
        Ok("msvc") => format!("/STACK:{STACK}"),
        _ => format!("-Wl,--stack,{STACK}"),
    };
    println!("cargo:rustc-link-arg-bins={arg}");
}
