//! The real shim on the main thread, which the GCMouse source needs, so
//! this test is `harness = false`. On macOS it starts on GCMouse: it lists
//! the mice, installs a handler on each one there is, and on drop sets
//! each back to null. CI's macOS runner has no mouse, so there it only
//! lists none; on a Mac with a mouse the handlers go on and off. Without a
//! run loop no callback comes, so nothing moves. Elsewhere it does not
//! start.

use std::process::ExitCode;

use eng_platform_gcmouse::{Motion, Shim};

fn main() -> ExitCode {
    let shim = Shim::start();
    if !cfg!(target_os = "macos") {
        assert!(shim.is_none(), "the shim runs on macOS only");
        println!("start: no shim off macOS: ok");
        return ExitCode::SUCCESS;
    }
    let mut shim = shim.expect("on macOS's main thread the shim starts");
    let callbacks = shim.callbacks();
    assert!(!shim.focused());
    assert_eq!(shim.drain(), Motion::default());
    // Focus looks for mice again.
    shim.set_focused(true);
    assert!(shim.focused());
    let moved = shim.drain();
    assert!(
        moved.callbacks <= callbacks.get(),
        "a drain sums only callbacks that came"
    );
    let mice = shim.mice();
    assert_eq!(mice, callbacks.mice());
    shim.set_focused(false);
    drop(shim);
    println!(
        "start: the shim started on GCMouse with {mice} mice and stopped; {} callbacks: ok",
        callbacks.get()
    );
    ExitCode::SUCCESS
}
