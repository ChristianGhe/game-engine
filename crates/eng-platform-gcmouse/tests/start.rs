//! The real shim on the main thread, which the GCMouse source needs, so
//! this test is `harness = false`. Elsewhere than macOS it checks that the
//! shim does not start.
//!
//! On macOS it starts the shim on GCMouse, gives it focus (so it looks for
//! mice again), takes focus away and drops it. Only `GCMouse::mice` is sure
//! to run: GCMouse lists mice only in an app, once AppKit's run loop has
//! run (under winit's event loop on an M2 Pro it listed the trackpad within
//! a second, and in this test, even after five seconds of the bare main run
//! loop, none). So `mouseInput`, the handler's install and its removal on
//! drop run in no test, here or on CI's macOS runner, which has no mouse;
//! they, the callbacks, their signs and units are checked by hand with
//! `eng-platform-winit`'s `examples/events.rs`. If GCMouse ever lists a
//! mouse here, the install path runs and the test says so.

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
    assert!(!shim.focused(), "the shim starts without focus");
    shim.set_focused(true);
    assert!(shim.focused());
    let mice = shim.mice();
    // Whatever came while focused is dropped with the focus.
    shim.set_focused(false);
    assert_eq!(shim.drain(), Motion::default(), "nothing after focus went");
    drop(shim);
    if mice == 0 {
        println!(
            "start: the shim started and stopped; GCMouse listed no mouse, so the handler's \
             install and removal did not run (check them with eng-platform-winit's events \
             example): ok"
        );
    } else {
        println!(
            "start: GCMouse listed {mice} mice; the handler went on each and came off on drop: ok"
        );
    }
    ExitCode::SUCCESS
}
