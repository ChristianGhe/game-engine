//! Rules for "proper" (rung 2, plan addition A2): what every first-person
//! shot of a scene must show, read from the scene's protected
//! `<map>.rules.toml` and judged over each shot's frame record
//! ([`crate::frame_record`]), as semantic assertions are judged over the
//! world. The testbed's `tests/scene_rules.rs` renders every shot and
//! judges it; a failure names the check, the shot, the object and the
//! values, tagged `[E-SCENE-RULE]`.
//!
//! ```toml
//! version = 1
//! map = "room"             # the shots' --map
//!
//! [rule-1]                 # each object a shot must see is
//! pixels = 50              # present: at least this many visible pixels;
//! contrast = 0.03          # distinct: its mean colour this far from its
//!                          # surroundings' (OKLab distance);
//! light = 0.2              # lit: its mean luminance this share of the
//!                          # surfaces'
//! [rule-2]
//! void = 0                 # at most this many pixels of nothing below the
//!                          # horizon,
//! sky = 0                  # and of nothing anywhere (class none)
//!
//! [rule-3]
//! share = 0.1              # floor in the lower third, ceiling or sky in the
//!                          # upper third, each at least this share of it
//!
//! [shot.room-spawn]        # one table for every first-person shot
//! must_see = ["marker"]    # objects by the record's `name`
//! except.rule-2-sky = { pixels = 2300, why = "the skylight" }
//! ```
//!
//! **The rules**, for each frame of the shot's record, each check by name:
//! 1. **Must see** (`rule-1`): each object the shot names in `must_see`
//!    (every object the frame names so; at least one must exist) has
//!    - at least `pixels` visible pixels (the record's `pixels`);
//!    - a *contrast* of at least `contrast`: the Euclidean distance in
//!      Björn Ottosson's OKLab (eng-render's [`Oklab`]; CSS Color 4 calls
//!      it deltaEOK, and about 0.02 is just noticeable) between its mean
//!      linear colour and its surroundings' (the record's `colour` and
//!      `surround_colour`: the pixels within 4 of its screen box that are
//!      not its own). It counts colour as well as lightness: a red marker
//!      before a grey pillar of the same luminance stands out;
//!    - a *light* of at least `light`: its mean luminance over the mean
//!      luminance of every surface in the frame (the record's `luminance`
//!      and `surface_luminance`, which leaves the sky out, so more sky in
//!      view does not dim it), so that an object drawn black, which may
//!      stand out well against a lit wall, still fails.
//! 2. **Nothing where the world should be**: the sky pass paints every
//!    pixel no surface covers, so the clear colour itself never shows; a
//!    hole in the world shows the sky instead.
//!    - `rule-2-void`: at most `void` pixels show nothing below the horizon
//!      (the record's `void`), where the ground always stands in front of
//!      the sky;
//!    - `rule-2-sky`: at most `sky` pixels show nothing at all (class
//!      `none` in the record's `classes`), so that a missing ceiling face
//!      fails too. A shot that sees the sky through an opening raises it.
//! 3. **Floor and ceiling**: class `floor` covers at least `share` of the
//!    lower third's pixels (`rule-3-floor`), and classes `ceiling` and
//!    `none` (the sky) together at least `share` of the upper third's
//!    (`rule-3-above`), by the record's `classes`.
//!
//! **Exceptions** are the rules file's, so the owner sees each, and each
//! carries a bound: a shot's `except.<check> = { pixels = N, why = "..." }`
//! raises `rule-2-void`'s or `rule-2-sky`'s limit to `N` for it, and
//! `except.<check> = { share = S, why = "..." }` lowers `rule-3-floor`'s or
//! `rule-3-above`'s to `S` (0 for none at all). A shot excuses itself from
//! rule 1 by not naming the object. A shot within the excused bound but
//! not the scene's limit passes, excused; past the bound it fails; and an
//! exception the shot does not need, passing the scene's own limit, is
//! stale and fails, so that none outlives its reason.

mod file;
mod finding;

use std::collections::BTreeMap;

use eng_math::sqrt;
use eng_render::{Oklab, SurfaceClass};

use crate::frame_record::{BANDS, FrameEntry, FrameRecord, ObjectRecord, RecordView};

pub use file::RULES_VERSION;
use finding::short;
pub use finding::{Finding, Measure, Outcome};

/// The checks a frame is judged by, in the order they are reported.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Check {
    /// Rule 1, for each object the shot must see.
    MustSee,
    /// Rule 2: nothing below the horizon.
    Void,
    /// Rule 2: nothing anywhere.
    Sky,
    /// Rule 3's lower third.
    Floor,
    /// Rule 3's upper third.
    Above,
}

impl Check {
    pub const ALL: [Self; 5] = [
        Self::MustSee,
        Self::Void,
        Self::Sky,
        Self::Floor,
        Self::Above,
    ];

    /// The rule's number: 1, 2 or 3.
    pub const fn rule(self) -> u8 {
        match self {
            Self::MustSee => 1,
            Self::Void | Self::Sky => 2,
            Self::Floor | Self::Above => 3,
        }
    }

    /// Its name in the rules file and in messages.
    pub const fn name(self) -> &'static str {
        match self {
            Self::MustSee => "rule-1",
            Self::Void => "rule-2-void",
            Self::Sky => "rule-2-sky",
            Self::Floor => "rule-3-floor",
            Self::Above => "rule-3-above",
        }
    }

    /// The check `name` names.
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|check| check.name() == name)
    }

    /// The measure a frame-wide check reads, and whether its value must be
    /// at most its limit (else at least); `None` for rule 1.
    pub const fn measure(self) -> Option<(&'static str, bool)> {
        match self {
            Self::MustSee => None,
            Self::Void | Self::Sky => Some(("pixels", true)),
            Self::Floor | Self::Above => Some(("share", false)),
        }
    }
}

/// Rule 1's limits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MustSee {
    pub pixels: u64,
    pub contrast: f64,
    pub light: f64,
}

/// A shot's exception to a check: the bound it is held to instead of the
/// scene's limit, and why.
#[derive(Clone, Debug, PartialEq)]
pub struct Exception {
    pub bound: f64,
    pub why: String,
}

/// What one shot must show, beyond the scene's rules: the objects it must
/// see, and its exceptions.
#[derive(Clone, Debug, PartialEq)]
pub struct ShotRules {
    pub must_see: Vec<String>,
    pub except: BTreeMap<Check, Exception>,
}

/// A `<map>.rules.toml`, read and checked.
#[derive(Clone, Debug, PartialEq)]
pub struct SceneRules {
    /// The map whose shots it judges.
    pub map: String,
    pub must_see: MustSee,
    /// Rule 2: the most pixels of nothing below the horizon, and anywhere.
    pub void: u64,
    pub sky: u64,
    /// Rule 3: the least share of a third.
    pub share: f64,
    /// Every first-person shot of the map, by name.
    pub shots: BTreeMap<String, ShotRules>,
}

impl SceneRules {
    /// A frame-wide check's limit for the whole scene.
    pub fn limit(&self, check: Check) -> f64 {
        match check {
            Check::MustSee => f64::NAN,
            Check::Void => self.void as f64,
            Check::Sky => self.sky as f64,
            Check::Floor | Check::Above => self.share,
        }
    }
}

/// The OKLab distance between two linear sRGB colours.
pub fn colour_distance(a: [f64; 3], b: [f64; 3]) -> f64 {
    let lab = |c: [f64; 3]| Oklab::from_linear_srgb(c.map(|v| v as f32));
    f64::from(sqrt(lab(a).distance_squared(lab(b))))
}

/// The pixels of each third of a `width` x `height` image, as the record
/// counts them (row `y` in third `3y / height`).
pub fn third_pixels(width: u32, height: u32) -> [u64; BANDS] {
    let mut rows = [0_u64; BANDS];
    for y in 0..u64::from(height) {
        rows[(y * BANDS as u64 / u64::from(height)) as usize] += 1;
    }
    rows.map(|r| r * u64::from(width))
}

impl SceneRules {
    /// Reads a rules file strictly (`origin` names it in errors): every
    /// key and no other, each limit in its range, each exception one of
    /// the frame-wide checks with a bound looser than the scene's and a
    /// reason.
    pub fn parse(text: &str, origin: &str) -> Result<Self, String> {
        file::parse(text, origin)
    }

    /// Judges every frame of `record`, a shot's of this scene's map named
    /// `shot`: every check's finding, passed or not. An error if the
    /// rules have no table for the shot, or the record is not first person.
    pub fn judge(&self, shot: &str, record: &FrameRecord) -> Result<Vec<Finding>, String> {
        let Some(rules) = self.shots.get(shot) else {
            return Err(format!(
                "[E-SCENE-RULE] the {} rules have no [shot.{shot}]: every first-person shot of \
                 the map needs its table",
                self.map
            ));
        };
        if record.view != RecordView::FirstPerson {
            return Err(format!(
                "[E-SCENE-RULE] {shot}: a {} record; the rules judge first-person frames",
                record.view.name()
            ));
        }
        let thirds = third_pixels(record.width, record.height);
        let mut findings = Vec::new();
        for frame in &record.frames {
            self.must_see(rules, frame, &mut findings);
            let void = frame.void.ok_or("a first-person frame without `void`")?;
            let classes = (frame.classes).ok_or("a first-person frame without classes")?;
            let count = |class: SurfaceClass, band: usize| classes[usize::from(class.code())][band];
            let share = |pixels: u64, band: usize| pixels as f64 / thirds[band].max(1) as f64;
            let (low, high) = (BANDS - 1, 0);
            let sky: u64 = classes[usize::from(SurfaceClass::None.code())].iter().sum();
            let above = count(SurfaceClass::Ceiling, high) + count(SurfaceClass::None, high);
            let values = [
                (
                    Check::Void,
                    void as f64,
                    "pixels showing nothing below the horizon",
                ),
                (Check::Sky, sky as f64, "pixels showing nothing: the sky"),
                (
                    Check::Floor,
                    share(count(SurfaceClass::Floor, low), low),
                    "of the lower third that is floor",
                ),
                (
                    Check::Above,
                    share(above, high),
                    "of the upper third that is ceiling or sky",
                ),
            ];
            for (check, value, note) in values {
                findings.push(self.frame_wide(rules, frame.tick, check, value, note));
            }
        }
        Ok(findings)
    }

    /// A frame-wide check of `value`, under the shot's exception if any.
    fn frame_wide(
        &self,
        rules: &ShotRules,
        tick: u64,
        check: Check,
        value: f64,
        note: &str,
    ) -> Finding {
        let (name, at_most) = check.measure().expect("a frame-wide check");
        let scene_limit = self.limit(check);
        let except = rules.except.get(&check);
        let measure = Measure {
            name,
            value: Some(value),
            limit: except.map_or(scene_limit, |e| e.bound),
            at_most,
        };
        let outcome = match except {
            None if measure.ok() => Outcome::Pass,
            None => Outcome::Fail,
            Some(e) if measure.passes(scene_limit) => Outcome::Stale {
                scene_limit,
                why: e.why.clone(),
            },
            Some(e) if measure.ok() => Outcome::Excused {
                scene_limit,
                why: e.why.clone(),
            },
            Some(_) => Outcome::Fail,
        };
        let note = match except {
            Some(e) if outcome == Outcome::Fail => {
                format!("{note}; past even the bound it is excused to: {}", e.why)
            }
            _ => note.to_owned(),
        };
        Finding {
            check,
            tick,
            object: None,
            measures: vec![measure],
            note,
            outcome,
        }
    }

    /// Rule 1 for each object `rules` must see in `frame`.
    fn must_see(&self, rules: &ShotRules, frame: &FrameEntry, findings: &mut Vec<Finding>) {
        let limits = self.must_see;
        for name in &rules.must_see {
            let named: Vec<&ObjectRecord> = (frame.objects.iter())
                .filter(|o| o.name.as_deref() == Some(name))
                .collect();
            if named.is_empty() {
                findings.push(Finding {
                    check: Check::MustSee,
                    tick: frame.tick,
                    object: Some(name.clone()),
                    measures: vec![Measure {
                        name: "pixels",
                        value: None,
                        limit: limits.pixels as f64,
                        at_most: false,
                    }],
                    note: "no object of the frame has this name: nothing asked to draw it".into(),
                    outcome: Outcome::Fail,
                });
            }
            for object in named {
                findings.push(must_see_one(limits, frame, object));
            }
        }
    }
}

/// Rule 1 for `object` of `frame`.
fn must_see_one(limits: MustSee, frame: &FrameEntry, object: &ObjectRecord) -> Finding {
    let contrast = match (object.colour, object.surround_colour) {
        (Some(own), Some(around)) => Some(colour_distance(own, around)),
        _ => None,
    };
    let light = match (object.luminance, frame.surface_luminance) {
        (Some(own), Some(surfaces)) if surfaces > 0.0 => Some(own / surfaces),
        _ => None,
    };
    let at_least = |name, value, limit| Measure {
        name,
        value,
        limit,
        at_most: false,
    };
    let measures = vec![
        at_least("pixels", Some(object.pixels as f64), limits.pixels as f64),
        at_least("contrast", contrast, limits.contrast),
        at_least("light", light, limits.light),
    ];
    let shown = |c: Option<[f64; 3]>| {
        c.map_or_else(
            || "none".to_owned(),
            |[r, g, b]| format!("[{}, {}, {}]", short(r), short(g), short(b)),
        )
    };
    let lum = |l: Option<f64>| l.map_or_else(|| "none".to_owned(), short);
    let note = format!(
        "colour {} against {}; luminance {} against the surfaces' {}",
        shown(object.colour),
        shown(object.surround_colour),
        lum(object.luminance),
        lum(frame.surface_luminance)
    );
    let passed = measures.iter().all(Measure::ok);
    Finding {
        check: Check::MustSee,
        tick: frame.tick,
        object: object.name.clone(),
        measures,
        note,
        outcome: if passed { Outcome::Pass } else { Outcome::Fail },
    }
}

#[cfg(test)]
mod tests;
