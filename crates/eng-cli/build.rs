//! Facts about this compilation for the version stamp (spec 6.13): the
//! target triple, the profile and the compiler. Facts about the source tree
//! (commits, dirty flag, commit time, CI run) come from `cargo xtask`
//! instead; see `src/version.rs`.

use std::env;
use std::process::Command;

const UNKNOWN: &str = "unknown";

fn main() {
    // Cargo sets both for every build script.
    let target = env::var("TARGET").unwrap_or_else(|_| UNKNOWN.into());
    // `debug` or `release`: the profile this one inherits from.
    let profile = env::var("PROFILE").unwrap_or_else(|_| UNKNOWN.into());
    let rustc = env::var_os("RUSTC").unwrap_or_else(|| "rustc".into());
    let rustc_version = Command::new(rustc)
        .arg("-V")
        .output()
        .ok()
        .filter(|out| out.status.success())
        .and_then(|out| String::from_utf8(out.stdout).ok())
        .map(|text| text.trim().to_string())
        .filter(|text| !text.is_empty() && !text.contains('\n'))
        .unwrap_or_else(|| UNKNOWN.into());
    println!("cargo::rustc-env=ENG_BUILD_TARGET={target}");
    println!("cargo::rustc-env=ENG_BUILD_PROFILE={profile}");
    println!("cargo::rustc-env=ENG_BUILD_RUSTC={rustc_version}");
    // A new compiler, target or profile reruns this anyway: each changes
    // the build's fingerprint or its output directory.
    println!("cargo::rerun-if-changed=build.rs");
}
