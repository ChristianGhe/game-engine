//! Opens a window and prints every platform event with its time, as the
//! app sees it; Escape or closing the window quits. A manual check of a
//! machine's input: `cargo run -p eng-platform-winit --example events`,
//! with `-- --seconds N` to quit after N seconds as well, `--grab` to grab
//! the cursor whenever the window has focus (as mouse look does), and
//! `--fullscreen` for borderless fullscreen. It prints what each request
//! answered and the window's refresh rate.
//!
//! On macOS raw motion comes from the GCMouse shim: each `MouseMotion` line
//! also shows the shim's callbacks since the one before and the motion
//! summed since the window showed, in counts, and once a second, while the
//! mouse moves, how many callbacks came in that second. That checks the
//! shim by hand (rung 2, T14's carry): moving right gives `dx > 0`, towards
//! you `dy > 0`; moving a known distance gives about the mouse's counts per
//! inch over that distance if the deltas are counts; and moving fast gives
//! more callbacks a second than the refresh rate if they are not coalesced.

use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use eng_platform_api::window::Window;
use eng_platform_api::{App, Control, Instant, Key, Platform, PlatformEvent, Wake};
use eng_platform_winit::{RawMotionCallbacks, WinitPlatform};

const SECOND: Duration = Duration::from_secs(1);

/// What the example has seen of the GCMouse shim (macOS).
struct Shim {
    callbacks: RawMotionCallbacks,
    /// The count at the last `MouseMotion` line.
    seen: u64,
    /// The motion summed since the window showed, in counts.
    sum: (f64, f64),
    /// The start of the current second, and the count then.
    second: Option<(Instant, u64)>,
}

impl Shim {
    fn motion(&mut self, dx: f64, dy: f64) -> String {
        let now = self.callbacks.get();
        let new = now - self.seen;
        self.seen = now;
        self.sum.0 += dx;
        self.sum.1 += dy;
        format!(
            "gcmouse: {new} callbacks; summed ({}, {}) counts",
            self.sum.0, self.sum.1
        )
    }

    /// Once a second, how many callbacks came in it; when the next second
    /// ends.
    fn tick(&mut self, now: Instant, refresh: Option<u32>) -> Instant {
        let count = self.callbacks.get();
        let (start, at_start) = *self.second.get_or_insert((now, count));
        let end = start + SECOND;
        if now < end {
            return end;
        }
        if count > at_start {
            println!(
                "{now} gcmouse: {} callbacks in the last second from {} mice; refresh rate {refresh:?} mHz",
                count - at_start,
                self.callbacks.mice()
            );
        }
        self.second = Some((now, count));
        now + SECOND
    }
}

struct Printer {
    quit_at: Option<Instant>,
    grab: bool,
    fullscreen: bool,
    window: Option<Arc<dyn Window>>,
    shim: Option<Shim>,
}

impl Printer {
    fn grab(&self, ctx: &mut dyn Control, grab: bool) {
        println!(
            "{} grab_cursor({grab}): {:?}",
            ctx.now(),
            ctx.grab_cursor(grab)
        );
    }
}

impl App for Printer {
    fn window(&mut self, ctx: &mut dyn Control, window: Arc<dyn Window>) {
        println!(
            "{} window: {:?}, refresh rate {:?} mHz",
            ctx.now(),
            window.size(),
            window.refresh_rate()
        );
        if self.fullscreen {
            println!(
                "{} set_fullscreen(true): {}",
                ctx.now(),
                ctx.set_fullscreen(true)
            );
        }
        if self.grab {
            self.grab(ctx, true);
        }
        self.window = Some(window);
    }

    fn event(&mut self, ctx: &mut dyn Control, event: PlatformEvent) {
        match (event, &mut self.shim) {
            (PlatformEvent::MouseMotion { dx, dy }, Some(shim)) => {
                println!("{} {event:?} {}", ctx.now(), shim.motion(dx, dy));
            }
            (PlatformEvent::FocusGained, Some(shim)) => {
                // The shim looked for mice just before.
                let mice = shim.callbacks.mice();
                println!("{} {event:?} gcmouse: {mice} mice", ctx.now());
            }
            _ => println!("{} {event:?}", ctx.now()),
        }
        match event {
            PlatformEvent::KeyDown(Key::Escape) | PlatformEvent::CloseRequested => ctx.exit(),
            PlatformEvent::FocusGained if self.grab => {
                self.grab(ctx, true);
                if let Some(window) = &self.window {
                    println!("{} refresh rate {:?} mHz", ctx.now(), window.refresh_rate());
                }
            }
            PlatformEvent::FocusLost if self.grab => self.grab(ctx, false),
            _ => {}
        }
    }

    fn frame(&mut self, ctx: &mut dyn Control) -> Wake {
        let now = ctx.now();
        if self.quit_at.is_some_and(|at| now >= at) {
            ctx.exit();
            return Wake::OnEvent;
        }
        let refresh = self.window.as_ref().and_then(|w| w.refresh_rate());
        let second = self.shim.as_mut().map(|shim| shim.tick(now, refresh));
        match (self.quit_at, second) {
            (Some(quit), Some(second)) => Wake::At(quit.min(second)),
            (Some(at), None) | (None, Some(at)) => Wake::At(at),
            (None, None) => Wake::OnEvent,
        }
    }

    fn stop(&mut self, _ctx: &dyn Control) {
        self.window = None;
    }
}

fn main() -> ExitCode {
    let mut printer = Printer {
        quit_at: None,
        grab: false,
        fullscreen: false,
        window: None,
        shim: None,
    };
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--seconds" => {
                let at = args
                    .next()
                    .and_then(|n| n.parse().ok())
                    .and_then(|n| Instant::ZERO.checked_add(Duration::from_secs(n)));
                match at {
                    Some(at) => printer.quit_at = Some(at),
                    None => return usage(),
                }
            }
            "--grab" => printer.grab = true,
            "--fullscreen" => printer.fullscreen = true,
            _ => return usage(),
        }
    }
    let result = WinitPlatform::new("eng-platform-winit events").and_then(|mut platform| {
        printer.shim = platform.raw_motion_callbacks().map(|callbacks| {
            println!("raw motion from the GCMouse shim");
            Shim {
                seen: callbacks.get(),
                callbacks,
                sum: (0.0, 0.0),
                second: None,
            }
        });
        platform.run(&mut printer)
    });
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn usage() -> ExitCode {
    eprintln!("usage: events [--seconds N] [--grab] [--fullscreen]");
    ExitCode::from(64)
}
