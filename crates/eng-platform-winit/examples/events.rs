//! Opens a window and prints every platform event with its time, as the
//! app sees it; Escape or closing the window quits. A manual check of a
//! machine's input: `cargo run -p eng-platform-winit --example events`,
//! with `-- --seconds N` to quit after N seconds as well, `--grab` to grab
//! the cursor whenever the window has focus (as mouse look does), and
//! `--fullscreen` for borderless fullscreen. It prints what each request
//! answered and the window's refresh rate.

use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use eng_platform_api::window::Window;
use eng_platform_api::{App, Control, Instant, Key, Platform, PlatformEvent, Wake};
use eng_platform_winit::WinitPlatform;

struct Printer {
    quit_at: Option<Instant>,
    grab: bool,
    fullscreen: bool,
    window: Option<Arc<dyn Window>>,
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
        println!("{} {event:?}", ctx.now());
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
        match self.quit_at {
            Some(at) if ctx.now() >= at => {
                ctx.exit();
                Wake::OnEvent
            }
            Some(at) => Wake::At(at),
            None => Wake::OnEvent,
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
    let result = WinitPlatform::new("eng-platform-winit events")
        .and_then(|mut platform| platform.run(&mut printer));
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
