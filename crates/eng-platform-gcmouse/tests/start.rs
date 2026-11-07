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
//!
//! As its own harness it speaks the part of libtest's command line that
//! `cargo test` and cargo-nextest use, as `eng-platform-winit`'s
//! `tests/conformance.rs` does: `--list` (with `--format terse` and
//! `--ignored`) names its one test, `start`; name filters, `--exact` and
//! `--skip` choose it or not; other flags are ignored. Without the listing,
//! nextest cannot make its test list and fails the whole run (CI run
//! 42403525706).

use std::process::ExitCode;

use eng_platform_gcmouse::{Motion, Shim};

/// The one test this harness holds.
const NAME: &str = "start";

/// Whether libtest's arguments `args` choose [`NAME`], and whether they
/// only ask for the list of tests.
fn chosen(args: impl IntoIterator<Item = String>) -> (bool, bool) {
    let (mut list, mut ignored, mut exact) = (false, false, false);
    let (mut filters, mut skips) = (Vec::new(), Vec::new());
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--list" => list = true,
            "--ignored" => ignored = true,
            "--exact" => exact = true,
            "--skip" => skips.extend(args.next()),
            // libtest's flags that take a value; the value means nothing here.
            "--format" | "--color" | "--test-threads" | "--logfile" | "--shuffle-seed" | "-Z" => {
                args.next();
            }
            flag if flag.starts_with('-') => {}
            filter => filters.push(filter.to_string()),
        }
    }
    let hit = |f: &String| {
        if exact {
            NAME == f
        } else {
            NAME.contains(f.as_str())
        }
    };
    // The test is not ignored, so `--ignored` chooses nothing.
    let selected =
        !ignored && (filters.is_empty() || filters.iter().any(hit)) && !skips.iter().any(hit);
    (selected, list)
}

fn main() -> ExitCode {
    let (selected, list) = chosen(std::env::args().skip(1));
    if list {
        if selected {
            println!("{NAME}: test");
        }
        return ExitCode::SUCCESS;
    }
    if !selected {
        return ExitCode::SUCCESS;
    }
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
