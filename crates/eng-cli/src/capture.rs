//! `shot` and `video`: frames of the game's top-down view, rendered
//! offscreen through eng-render from a tick's state at alpha 1, so they are
//! exact (plan, "One frame"), and written as a PNG or an animated PNG.
//!
//! - The GPU opens before the simulation runs. Without an adapter, or with
//!   a device that will not open, the command exits 64 with eng-gpu's
//!   message (`[E-NO-ADAPTER]`, `[E-GPU]`; plan decision 4), writing nothing.
//! - Once the GPU is open, a frame that fails to render is a bug in the
//!   engine or the game's extraction (`[E-SCENE]`, `[E-GPU]`): it panics, so
//!   the panic hook writes `crash.json` with the tick and seed (exit 12).
//! - Frames are kept in memory and written once the run reaches its last
//!   tick, so a run that stops early leaves no partial file.

use std::path::PathBuf;

use eng_gpu::{FrameCounters, Gpu, GpuOptions, GpuReport, Targets};
use eng_render::{Renderer, TopDownScene};
use eng_sim::Sim;

use crate::args::{CaptureArgs, Frames};
use crate::image_file::{Apng, encode_png, frame_delay};
use crate::{GameDef, TopDownFrame};

/// What the report and the console say about the frames.
pub(crate) struct CaptureSummary {
    pub report: GpuReport,
    pub width: u32,
    pub height: u32,
    pub frames: u64,
    pub out: PathBuf,
}

enum Sink {
    /// The PNG, once its frame is rendered.
    Shot(Option<Vec<u8>>),
    Video(Apng),
}

/// The GPU, the renderer and the frames rendered so far.
pub(crate) struct Capture {
    args: CaptureArgs,
    last_tick: u64,
    top_down: fn(&TopDownFrame<'_>) -> TopDownScene,
    gpu: Gpu,
    renderer: Renderer,
    targets: Targets,
    sink: Sink,
    frames: u64,
}

impl Capture {
    /// Opens the GPU for a run of `ticks` ticks. The error is the line to
    /// print, with its tag; the command then exits 64.
    pub fn open(game: &GameDef, args: &CaptureArgs, ticks: u64) -> Result<Self, String> {
        let top_down = game.top_down.ok_or_else(|| {
            format!(
                "[E-USAGE] {} has no top-down view to render (GameDef::top_down)",
                game.name
            )
        })?;
        let sink = match args.frames {
            Frames::Last => Sink::Shot(None),
            Frames::Every(every) => {
                let delay =
                    frame_delay(every, game.tick_rate).map_err(|e| format!("[E-USAGE] {e}"))?;
                // At most MAX_VIDEO_FRAMES (args), so it fits.
                let count = u32::try_from(ticks / every + 1).expect("frames are capped");
                Sink::Video(Apng::new(args.width, args.height, count, delay)?)
            }
        };
        let gpu = Gpu::new(&GpuOptions::from_env()).map_err(|e| e.to_string())?;
        let renderer = Renderer::new(&gpu).unwrap_or_else(|e| panic!("{e}"));
        let targets = Targets::new(&gpu, args.width, args.height).map_err(|e| e.to_string())?;
        Ok(Self {
            args: args.clone(),
            last_tick: ticks,
            top_down,
            gpu,
            renderer,
            targets,
            sink,
            frames: 0,
        })
    }

    /// Whether the state at `tick` is a frame.
    pub fn wants(&self, tick: u64) -> bool {
        match self.args.frames {
            Frames::Last => tick == self.last_tick,
            Frames::Every(every) => tick.is_multiple_of(every),
        }
    }

    /// Renders `sim`'s state, keeps the frame and returns its counters.
    ///
    /// # Panics
    ///
    /// If the frame cannot be rendered or read back: a bug, see the module
    /// docs.
    pub fn frame(&mut self, sim: &Sim) -> FrameCounters {
        let tick = sim.tick();
        let scene = (self.top_down)(&TopDownFrame {
            previous: sim,
            current: sim,
            alpha: 1.0,
            width: self.args.width,
            height: self.args.height,
        });
        let counters = self
            .renderer
            .render(&self.gpu, &self.targets, &scene)
            .unwrap_or_else(|e| panic!("{e} (frame at {tick})"));
        let image = self
            .targets
            .read_output(&self.gpu)
            .unwrap_or_else(|e| panic!("{e} (reading back the frame at {tick})"));
        let kept = match &mut self.sink {
            Sink::Shot(png) => encode_png(&image).map(|bytes| *png = Some(bytes)),
            Sink::Video(apng) => apng.push(&image),
        };
        kept.unwrap_or_else(|e| panic!("{e} (frame at {tick})"));
        self.frames += 1;
        counters
    }

    pub fn summary(&self) -> CaptureSummary {
        CaptureSummary {
            report: self.gpu.report().clone(),
            width: self.args.width,
            height: self.args.height,
            frames: self.frames,
            out: self.args.out.clone(),
        }
    }

    /// The file's path and bytes, once every frame is in.
    ///
    /// # Panics
    ///
    /// If the run reached its last tick without every frame: a bug.
    pub fn finish(self) -> (PathBuf, Vec<u8>) {
        let bytes = match self.sink {
            Sink::Shot(png) => png.expect("the shot's tick was rendered"),
            Sink::Video(apng) => apng.finish().unwrap_or_else(|e| panic!("{e}")),
        };
        (self.args.out, bytes)
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use eng_sim::{ScheduleError, Sim, SimBuilder, SimConfig};

    use super::Capture;
    use crate::GameDef;
    use crate::args::{CaptureArgs, Frames};

    fn build(config: SimConfig) -> Result<Sim, ScheduleError> {
        SimBuilder::new(config).build()
    }

    fn game(tick_rate: u32, view: bool) -> GameDef {
        fn empty(frame: &crate::TopDownFrame<'_>) -> eng_render::TopDownScene {
            let _ = frame;
            unreachable!("never rendered here")
        }
        GameDef {
            name: "viewless",
            version: "0",
            build_sha: crate::UNKNOWN,
            tick_rate,
            build,
            top_down: view.then_some(empty as fn(&crate::TopDownFrame<'_>) -> _),
        }
    }

    fn args(frames: Frames) -> CaptureArgs {
        CaptureArgs {
            frames,
            out: PathBuf::from("never-written.png"),
            width: 8,
            height: 8,
        }
    }

    /// Both are refused before the GPU opens, so no adapter is needed.
    #[test]
    fn what_cannot_be_rendered_is_refused_first() {
        let Err(err) = Capture::open(&game(60, false), &args(Frames::Last), 5) else {
            panic!("a game without a view renders nothing");
        };
        assert_eq!(
            err,
            "[E-USAGE] viewless has no top-down view to render (GameDef::top_down)"
        );
        let Err(err) = Capture::open(&game(65_537, true), &args(Frames::Every(1)), 5) else {
            panic!("1/65537 s does not fit APNG's fraction");
        };
        assert!(err.starts_with("[E-USAGE] a frame delay"), "{err}");
    }
}
