#[cfg(unix)]
use std::env;
#[cfg(unix)]
use std::path::PathBuf;

use vergen::Emitter;

fn main() -> anyhow::Result<()> {
    add_libghostty_vt_rpath();
    Emitter::default().emit()
}

#[cfg(unix)]
fn add_libghostty_vt_rpath() {
    println!("cargo:rerun-if-env-changed=DEP_GHOSTTY_VT_INCLUDE");

    let Ok(include_dir) = env::var("DEP_GHOSTTY_VT_INCLUDE") else {
        return;
    };
    let include_dir = PathBuf::from(include_dir);
    let Some(prefix_dir) = include_dir.parent() else {
        return;
    };
    let lib_dir = prefix_dir.join("lib");

    // libghostty-vt-sys 0.1.1 links the vendored native library dynamically. The rpath keeps
    // `cargo run` and local release builds runnable without requiring users to export a
    // platform-specific library-path environment variable.
    println!(
        "cargo:rustc-link-arg-bin=betamax=-Wl,-rpath,{}",
        lib_dir.display()
    );
}

#[cfg(not(unix))]
fn add_libghostty_vt_rpath() {
    println!("cargo:rerun-if-env-changed=DEP_GHOSTTY_VT_INCLUDE");
}
