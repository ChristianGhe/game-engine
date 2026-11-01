use eng_core::{PlayerCommand, Tick};
use eng_schema_json::{Authored, LoadError, load_toml};
use serde::Deserialize;

/// An input script, version 1: segments of held input for one player.
///
/// ```toml
/// version = 1
///
/// [[segment]]
/// ticks = 60        # hold for 60 ticks (one second at 60 Hz)
/// forward = 127     # -128..=127; side works the same, positive is right
/// yaw_deg = 90.0    # absolute; held until a later segment changes it
/// ```
///
/// After the last segment, angles hold and everything else is idle.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputScript {
    pub version: u32,
    #[serde(default)]
    pub segment: Vec<Segment>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Segment {
    pub ticks: u32,
    #[serde(default)]
    pub forward: i8,
    #[serde(default)]
    pub side: i8,
    /// Absolute yaw in degrees, counter-clockwise from +X seen from above.
    pub yaw_deg: Option<f64>,
    /// Absolute pitch in degrees, positive up.
    pub pitch_deg: Option<f64>,
    #[serde(default)]
    pub buttons: u32,
    #[serde(default)]
    pub select: u8,
}

impl Authored for InputScript {
    const KIND: &'static str = "input script";
    const VERSION: u32 = 1;
}

/// Quantizes degrees to a 16-bit binary angle (65536 per turn), rounding
/// to nearest.
///
/// # Panics
/// If `deg` is not finite; [`InputScript::parse`] rejects such scripts.
pub fn degrees_to_angle(deg: f64) -> u16 {
    assert!(deg.is_finite(), "angle must be finite");
    let turns = deg.rem_euclid(360.0) / 360.0;
    ((turns * 65536.0).round() as u32 & 0xffff) as u16
}

impl InputScript {
    /// Loads and validates a script. `origin` names it in errors.
    pub fn parse(text: &str, origin: &str) -> Result<Self, LoadError> {
        let script: Self = load_toml(text, origin)?;
        for (i, seg) in script.segment.iter().enumerate() {
            let fail = |message: String| LoadError {
                origin: origin.to_string(),
                message: format!("segment {}: {message}", i + 1),
            };
            if seg.ticks == 0 {
                return Err(fail("ticks must be at least 1".into()));
            }
            for (name, value) in [("yaw_deg", seg.yaw_deg), ("pitch_deg", seg.pitch_deg)] {
                if value.is_some_and(|v| !v.is_finite()) {
                    return Err(fail(format!("{name} must be finite")));
                }
            }
        }
        Ok(script)
    }

    /// The number of ticks the segments cover.
    pub fn len_ticks(&self) -> u64 {
        self.segment.iter().map(|s| u64::from(s.ticks)).sum()
    }

    /// Commands for ticks `1..=ticks`.
    pub fn commands(&self, ticks: u64) -> Vec<PlayerCommand> {
        self.iter_commands()
            .take(usize::try_from(ticks).unwrap_or(usize::MAX))
            .collect()
    }

    /// Commands for ticks 1, 2, 3, ... without end: after the last segment,
    /// angles hold and everything else is idle.
    pub fn iter_commands(&self) -> impl Iterator<Item = PlayerCommand> + '_ {
        let mut cursor = Cursor::default();
        std::iter::from_fn(move || Some(cursor.next(&self.segment)))
    }

    /// The same commands as [`iter_commands`](Self::iter_commands), from an
    /// iterator that keeps the script and borrows nothing.
    pub fn into_commands(self) -> impl Iterator<Item = PlayerCommand> {
        let mut cursor = Cursor::default();
        std::iter::from_fn(move || Some(cursor.next(&self.segment)))
    }
}

/// How far a script's commands have got: the last tick given, the segment
/// and the ticks given from it, and the angles held.
#[derive(Default)]
struct Cursor {
    tick: u64,
    segment: usize,
    used: u32,
    yaw: u16,
    pitch: u16,
}

impl Cursor {
    /// The next tick's command.
    fn next(&mut self, segments: &[Segment]) -> PlayerCommand {
        self.tick += 1;
        let mut cmd = PlayerCommand::idle(Tick(self.tick));
        while segments
            .get(self.segment)
            .is_some_and(|seg| self.used >= seg.ticks)
        {
            self.segment += 1;
            self.used = 0;
        }
        if let Some(seg) = segments.get(self.segment) {
            self.used += 1;
            self.yaw = seg.yaw_deg.map_or(self.yaw, degrees_to_angle);
            self.pitch = seg.pitch_deg.map_or(self.pitch, degrees_to_angle);
            cmd.forward = seg.forward;
            cmd.side = seg.side;
            cmd.buttons = seg.buttons;
            cmd.select = seg.select;
        }
        cmd.view_yaw = self.yaw;
        cmd.view_pitch = self.pitch;
        cmd
    }
}
