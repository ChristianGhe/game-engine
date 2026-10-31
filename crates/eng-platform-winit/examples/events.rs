//! Opens a window and prints every platform event with its time, as the
//! app sees it; Escape or closing the window quits. A manual check of a
//! machine's input: `cargo run -p eng-platform-winit --example events`,
//! with `-- --seconds N` to quit after N seconds as well.

use std::process::ExitCode;
use std::time::Duration;

use eng_platform_api::{App, Control, Instant, Key, Platform, PlatformEvent, Wake};
use eng_platform_winit::WinitPlatform;

struct Printer {
    quit_at: Option<Instant>,
}

impl App for Printer {
    fn event(&mut self, ctx: &mut dyn Control, event: PlatformEvent) {
        println!("{} {event:?}", ctx.now());
        if matches!(
            event,
            PlatformEvent::KeyDown(Key::Escape) | PlatformEvent::CloseRequested
        ) {
            ctx.exit();
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
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let quit_at = match args.as_slice() {
        [] => None,
        [flag, n] if flag == "--seconds" => {
            match n
                .parse()
                .ok()
                .and_then(|n| Instant::ZERO.checked_add(Duration::from_secs(n)))
            {
                Some(at) => Some(at),
                None => return usage(),
            }
        }
        _ => return usage(),
    };
    let result = WinitPlatform::new("eng-platform-winit events")
        .and_then(|mut platform| platform.run(&mut Printer { quit_at }));
    match result {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("{e}");
            ExitCode::FAILURE
        }
    }
}

fn usage() -> ExitCode {
    eprintln!("usage: events [--seconds N]");
    ExitCode::from(64)
}
