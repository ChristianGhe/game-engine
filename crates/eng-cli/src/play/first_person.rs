//! A first-person `play` (rung 2, T15): its flags, checked and loaded
//! before a window opens, and what its window did, for the report.
//!
//! - **Which maps.** The first-person flags apply to a map whose view is
//!   [`MapView::FirstPerson`] only. On another map each is a usage error
//!   (`[E-USAGE]`, exit 64) before the window: 1b's dot map plays exactly
//!   as it did.
//! - **The flags** ([`FirstPersonPlay`]): `--windowed`, `--fov`, `--look`
//!   (a file, [`LookSwitches`]), and mouse look's `--sensitivity`,
//!   `--pitch-limit` and `--invert-pitch` over [`LookSettings::DEFAULT`].
//!   A look file that does not load is a usage error too.
//! - **The report** gains a `window` object ([`PlayWindow`]): fullscreen
//!   asked for and taken, the grab the platform granted, focus losses, the
//!   refresh rate and the frames' period, the last frame's lens, and on
//!   macOS the GCMouse shim's callback rate (criteria 10 and 25).

use eng_input::LookSettings;
use eng_math::BAngle;
use eng_platform_api::CursorGrab;
use eng_schema_json::JsonValue;

use crate::GameDef;
use crate::args::FirstPersonFlags;
use crate::look_file::LookSwitches;
use crate::maps::MapChoice;
use crate::run::read_text;
use crate::view::{FirstPersonView, MapView};

/// A first-person `play`'s own settings: its map's view and its flags.
#[derive(Debug, Clone, Copy)]
pub struct FirstPersonPlay {
    /// The map's view: its world, loaded once, and each frame's scene.
    pub view: FirstPersonView,
    /// `--windowed`: a window; without it, borderless fullscreen.
    pub windowed: bool,
    /// `--fov`: the vertical field of view; `None` keeps the scene's.
    pub fov_y: Option<BAngle>,
    /// `--look`: the switches set over the scene's look.
    pub look: LookSwitches,
    /// Mouse look: `--sensitivity`, `--pitch-limit` and `--invert-pitch`
    /// over the defaults (plan decision 9).
    pub mouse: LookSettings,
}

/// What a first-person window did, for the report's `window` object. The
/// app fills it in and gives it to [`crate::PlayRun::observe_window`]
/// before `finish`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PlayWindow {
    /// Borderless fullscreen was asked for (no `--windowed`).
    pub fullscreen_asked: bool,
    /// The platform took the request (`Control::set_fullscreen`'s answer);
    /// the change itself may land later.
    pub fullscreen: bool,
    /// The cursor grab was asked for, at least once: a live run whose
    /// window opened or gained focus. A scripted run never asks. With
    /// [`PlayWindow::grab`] it tells apart a grab never asked for, one
    /// asked for and refused, and one waiting for a focus that never came.
    pub grab_asked: bool,
    /// The grab the platform last granted (`Control::grab_cursor`'s
    /// answer), `None` if it never granted one.
    pub grab: CursorGrab,
    /// The grab in force at the end, as the app last heard: `None` after a
    /// focus loss, which frees the cursor.
    pub grab_at_end: CursorGrab,
    /// Times the window lost focus, each freeing the cursor. winit reports
    /// one before a new window's first `FocusGained` (ADR-0022, Amendment
    /// 1), so a window that opened is at 1 or more.
    pub focus_lost: u64,
    /// The window's refresh rate as last read, in millihertz; `None` when
    /// the platform cannot tell, or there was no window.
    pub refresh_rate_mhz: Option<u32>,
    /// How far apart frames were paced, in nanoseconds: a refresh period
    /// when the rate is known, else a tick.
    pub frame_period_ns: u64,
    /// The last frame drawn, if one was.
    pub frame: Option<FrameLens>,
    /// The GCMouse shim's callbacks (macOS); `None` where it does not run.
    pub gcmouse: Option<MouseRate>,
}

/// The lens of a first-person frame as drawn.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameLens {
    /// The vertical field of view.
    pub fov_y: BAngle,
    /// The near plane, in meters (eng-app keeps the near rectangle inside
    /// the hull).
    pub near: f32,
    /// Window pixels across and down per texel (low resolution).
    pub upscale: u32,
}

/// The GCMouse shim's callbacks during a run (criterion 25): the mouse's
/// own rate, which raw motion events do not show (they follow the wakes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct MouseRate {
    /// Callbacks while the window had focus.
    pub callbacks: u64,
    /// The most callbacks per second over any span of at least a second
    /// with focus throughout, rounded down; 0 with no full span.
    pub peak_per_second: u64,
    /// How long the window had focus while the count was read, in
    /// milliseconds.
    pub focused_ms: u64,
}

/// The first-person settings of a run on `map`, or `None` for a map that
/// is not first person; the error is the usage message.
pub(crate) fn settle(
    game: &GameDef,
    map: &MapChoice,
    flags: &FirstPersonFlags,
) -> Result<Option<FirstPersonPlay>, String> {
    let MapView::FirstPerson(view) = map.view() else {
        return match flags.first_given() {
            Some(flag) => Err(format!(
                "{flag} is for a first-person map; {}'s map {:?} {}",
                game.name,
                map.name(),
                match map.view() {
                    MapView::None => "has no view".to_string(),
                    other => format!("is {}", other.name()),
                }
            )),
            None => Ok(None),
        };
    };
    let look = match &flags.look {
        Some(path) => {
            let text = read_text(path)?;
            LookSwitches::parse(&text, &path.display().to_string()).map_err(|e| e.to_string())?
        }
        None => LookSwitches::default(),
    };
    let defaults = LookSettings::DEFAULT;
    Ok(Some(FirstPersonPlay {
        view,
        windowed: flags.windowed,
        fov_y: flags.fov_y,
        look,
        mouse: LookSettings {
            sensitivity_microdeg: flags
                .sensitivity_microdeg
                .unwrap_or(defaults.sensitivity_microdeg),
            invert_pitch: flags.invert_pitch,
            pitch_limit: flags.pitch_limit.unwrap_or(defaults.pitch_limit),
        },
    }))
}

/// Degrees in a binary angle, for the report.
fn degrees(angle: BAngle) -> f64 {
    f64::from(angle.0) * 360.0 / 4_294_967_296.0
}

impl PlayWindow {
    /// The report's `window` object.
    pub(crate) fn to_json(self) -> JsonValue {
        let optional = |v: Option<u64>| v.map_or(JsonValue::Null, JsonValue::U64);
        let frame = self.frame.map_or(JsonValue::Null, |f| {
            JsonValue::object([
                ("fov_y_deg", JsonValue::F64(degrees(f.fov_y))),
                ("near_m", JsonValue::F64(f64::from(f.near))),
                ("upscale", JsonValue::U64(f.upscale.into())),
            ])
        });
        let gcmouse = self.gcmouse.map_or(JsonValue::Null, |m| {
            JsonValue::object([
                ("callbacks", JsonValue::U64(m.callbacks)),
                ("peak_per_second", JsonValue::U64(m.peak_per_second)),
                ("focused_ms", JsonValue::U64(m.focused_ms)),
            ])
        });
        JsonValue::object([
            ("view", JsonValue::str("first-person")),
            ("fullscreen_asked", JsonValue::Bool(self.fullscreen_asked)),
            ("fullscreen", JsonValue::Bool(self.fullscreen)),
            ("grab_asked", JsonValue::Bool(self.grab_asked)),
            ("grab", JsonValue::str(self.grab.name())),
            ("grab_at_end", JsonValue::str(self.grab_at_end.name())),
            ("focus_lost", JsonValue::U64(self.focus_lost)),
            (
                "refresh_rate_mhz",
                optional(self.refresh_rate_mhz.map(u64::from)),
            ),
            ("frame_period_ns", JsonValue::U64(self.frame_period_ns)),
            ("frame", frame),
            ("gcmouse", gcmouse),
        ])
    }

    /// The console's line, such as `window: fullscreen, grab locked,
    /// 120 Hz; GCMouse peak 1000/s`.
    pub(crate) fn console_line(self) -> String {
        let mode = match (self.fullscreen_asked, self.fullscreen) {
            (false, _) => "windowed",
            (true, true) => "fullscreen",
            (true, false) => "fullscreen refused",
        };
        let rate = match self.refresh_rate_mhz {
            Some(mhz) => format!("{}.{:03} Hz", mhz / 1000, mhz % 1000),
            None => "refresh rate unknown".to_string(),
        };
        let grab = if self.grab_asked {
            self.grab.name()
        } else {
            "not asked"
        };
        let mut line = format!(
            "window: {mode}, grab {grab}, {rate}, focus lost {}",
            self.focus_lost
        );
        if let Some(m) = self.gcmouse {
            line.push_str(&format!(
                "; GCMouse peak {}/s, {} callbacks in {} ms focused",
                m.peak_per_second, m.callbacks, m.focused_ms
            ));
        }
        line
    }
}

#[cfg(test)]
mod tests {
    use eng_input::LookSettings;
    use eng_math::BAngle;
    use eng_platform_api::CursorGrab;
    use eng_sim::{ScheduleError, Sim, SimBuilder, SimConfig};

    use super::{FrameLens, MouseRate, PlayWindow, settle};
    use crate::args::FirstPersonFlags;
    use crate::maps::MapChoice;
    use crate::view::MapView;
    use crate::{GameDef, GameMap, UNKNOWN};

    fn build(config: SimConfig) -> Result<Sim, ScheduleError> {
        SimBuilder::new(config).build()
    }

    const GAME: GameDef = GameDef {
        name: "fp",
        version: "0",
        build_sha: UNKNOWN,
        tick_rate: 60,
        build,
        top_down: None,
        view_yaw: 0,
    };

    /// A map named `name` of `view`; `settle` builds no sim.
    fn map(name: &'static str, view: MapView) -> MapChoice {
        MapChoice {
            map: GameMap { name, build, view },
            first: "plain",
        }
    }

    #[test]
    fn a_first_person_map_takes_its_flags_over_the_defaults() {
        let room = map("room", crate::view::tests::FIRST_PERSON);
        let fp = settle(&GAME, &room, &FirstPersonFlags::default())
            .unwrap()
            .expect("first person");
        assert!(!fp.windowed && fp.fov_y.is_none());
        assert_eq!(fp.look, crate::LookSwitches::default());
        assert_eq!(fp.mouse, LookSettings::DEFAULT);

        let dir = std::env::temp_dir().join(format!("eng-cli-fp-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let look = dir.join("look.toml");
        std::fs::write(&look, "version = 1\nfog = false\n").unwrap();
        let flags = FirstPersonFlags {
            windowed: true,
            fov_y: Some(BAngle::QUARTER),
            look: Some(look.clone()),
            sensitivity_microdeg: Some(22_000),
            pitch_limit: Some(8_192),
            invert_pitch: true,
        };
        let fp = settle(&GAME, &room, &flags).unwrap().unwrap();
        assert!(fp.windowed);
        assert_eq!(fp.fov_y, Some(BAngle::QUARTER));
        assert_eq!(fp.look.fog, Some(false));
        assert_eq!(
            fp.mouse,
            LookSettings {
                sensitivity_microdeg: 22_000,
                invert_pitch: true,
                pitch_limit: 8_192,
            }
        );
        // A look file that does not load is refused, naming it.
        std::fs::write(&look, "version = 1\nfogg = false\n").unwrap();
        let err = settle(&GAME, &room, &flags).unwrap_err();
        assert!(err.starts_with("[E-LOAD]") && err.contains("fogg"), "{err}");
        let missing = FirstPersonFlags {
            look: Some(dir.join("missing.toml")),
            ..FirstPersonFlags::default()
        };
        let err = settle(&GAME, &room, &missing).unwrap_err();
        assert!(err.contains("missing.toml"), "{err}");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn another_map_refuses_every_first_person_flag() {
        let dots = map("dots", MapView::TopDown);
        assert!(matches!(
            settle(&GAME, &dots, &FirstPersonFlags::default()),
            Ok(None)
        ));
        let one = |set: fn(&mut FirstPersonFlags)| {
            let mut flags = FirstPersonFlags::default();
            set(&mut flags);
            settle(&GAME, &dots, &flags).map(|_| ()).unwrap_err()
        };
        let messages = [
            one(|f| f.windowed = true),
            one(|f| f.fov_y = Some(BAngle::QUARTER)),
            one(|f| f.look = Some("l.toml".into())),
            one(|f| f.sensitivity_microdeg = Some(1)),
            one(|f| f.pitch_limit = Some(1)),
            one(|f| f.invert_pitch = true),
        ];
        let flags = [
            "--windowed",
            "--fov",
            "--look",
            "--sensitivity",
            "--pitch-limit",
            "--invert-pitch",
        ];
        for (message, flag) in messages.iter().zip(flags) {
            assert_eq!(
                message,
                &format!("{flag} is for a first-person map; fp's map \"dots\" is top-down")
            );
        }
        let none = map("blank", MapView::None);
        let err = settle(
            &GAME,
            &none,
            &FirstPersonFlags {
                windowed: true,
                ..FirstPersonFlags::default()
            },
        );
        assert_eq!(
            err.map(|_| ()).unwrap_err(),
            "--windowed is for a first-person map; fp's map \"blank\" has no view"
        );
    }

    #[test]
    fn the_window_object_names_the_grab_fullscreen_and_rates() {
        let window = PlayWindow {
            fullscreen_asked: true,
            fullscreen: true,
            grab_asked: true,
            grab: CursorGrab::Locked,
            grab_at_end: CursorGrab::None,
            focus_lost: 2,
            refresh_rate_mhz: Some(119_880),
            frame_period_ns: 7_820_000,
            frame: Some(FrameLens {
                fov_y: BAngle::QUARTER,
                near: 0.1,
                upscale: 4,
            }),
            gcmouse: Some(MouseRate {
                callbacks: 9000,
                peak_per_second: 1000,
                focused_ms: 7_500,
            }),
        };
        let json = window.to_json().to_pretty();
        for key in [
            "\"grab_asked\": true",
            "\"grab\": \"locked\"",
            "\"grab_at_end\": \"none\"",
            "\"fullscreen\": true",
            "\"refresh_rate_mhz\": 119880",
            "\"fov_y_deg\": 90",
            "\"upscale\": 4",
            "\"peak_per_second\": 1000",
        ] {
            assert!(json.contains(key), "{key} in {json}");
        }
        assert_eq!(
            window.console_line(),
            "window: fullscreen, grab locked, 119.880 Hz, focus lost 2; GCMouse peak 1000/s, 9000 \
             callbacks in 7500 ms focused"
        );
        let plain = PlayWindow {
            fullscreen_asked: false,
            fullscreen: false,
            grab_asked: false,
            grab: CursorGrab::None,
            grab_at_end: CursorGrab::None,
            focus_lost: 0,
            refresh_rate_mhz: None,
            frame_period_ns: 16_666_666,
            frame: None,
            gcmouse: None,
        };
        let json = plain.to_json().to_pretty();
        assert!(json.contains("\"frame\": null") && json.contains("\"gcmouse\": null"));
        assert!(json.contains("\"grab_asked\": false"), "{json}");
        assert_eq!(
            plain.console_line(),
            "window: windowed, grab not asked, refresh rate unknown, focus lost 0"
        );
        // Asked for and never granted: refused, or no focus came.
        let waiting = PlayWindow {
            grab_asked: true,
            ..plain
        };
        assert_eq!(
            waiting.console_line(),
            "window: windowed, grab none, refresh rate unknown, focus lost 0"
        );
    }
}
