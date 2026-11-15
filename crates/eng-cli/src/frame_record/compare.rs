//! Two records of the same frames from two adapters, compared within
//! [`Tolerances`] ([`FrameRecord::compare`]). What the CPU decides must be
//! equal: the view, the size, the ticks, the camera, the objects asked for
//! (keys, names and world boxes), the unlisted keys and `invalid_info`.
//! What the GPU draws may differ a little: rasterizers place edge pixels
//! differently and round colours apart, which the palette pass turns into
//! a neighbouring palette entry here and there.

use eng_math::{max_f64, min_f64};
use eng_schema_json::JsonValue;

use super::{FrameEntry, FrameRecord, ObjectRecord, Probe, class_name};

/// How far two adapters' records of one shot may differ.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tolerances {
    /// Any pixel count (an object's, an unlisted key's, a class's in one
    /// third) may be apart by this many, and more as below.
    pub pixels: u64,
    /// An object's by this many more per pixel of its screen box's
    /// perimeter (the larger of the two boxes): rasterizers differ along
    /// edges, so a missing small object still fails.
    pub edge_pixels: f64,
    /// A class's in one third by this share more of the larger count.
    pub class_fraction: f64,
    /// Each edge of an object's screen box, in pixels.
    pub screen_box: u32,
    /// An object with fewer pixels than this on either side has its
    /// luminance, surroundings and contrast left uncompared: a few edge
    /// pixels decide them.
    pub min_pixels: u64,
    /// An object's and the image's mean luminance, and an object's
    /// surroundings', apart by at most this.
    pub luminance: f64,
    /// An object's contrast, apart by at most this plus what `luminance`
    /// allows it: Michelson's contrast moves by up to `2 t / (L + S)` when
    /// its luminance `L` and surroundings `S` each move by `t`, so a dark
    /// object's contrast is held more loosely ([`contrast_tolerance`]).
    pub contrast: f64,
    /// The share of the image's pixels that may sit in another histogram
    /// bin: half the sum of the bins' differences, over the pixels.
    pub histogram: f64,
}

impl Tolerances {
    /// From the room's seven first-person shots on Metal (Apple M2 Pro)
    /// against the pinned lavapipe, 2026-11-09: every object's pixels
    /// within 1, every screen box the same, luminance within 0.0035, at
    /// most 1.2% of the pixels in another histogram bin (`room-look-off`,
    /// without the palette), probes within 3% of their depth tolerance. So
    /// about three times what was seen; the testbed's
    /// `frame-tolerances.toml` holds the same `[default]` and can raise a
    /// shot's.
    pub const DEFAULT: Self = Self {
        pixels: 2,
        edge_pixels: 0.1,
        class_fraction: 0.01,
        screen_box: 1,
        min_pixels: 50,
        luminance: 0.01,
        contrast: 0.02,
        histogram: 0.03,
    };
}

/// How far two contrasts of an object may be apart: `tolerances.contrast`
/// plus `2 t / (L + S)` for `t` the luminance tolerance and `L + S` the
/// smaller of the two sides' luminance plus surroundings.
pub fn contrast_tolerance(tolerances: &Tolerances, a: &ObjectRecord, b: &ObjectRecord) -> f64 {
    let sum = |o: &ObjectRecord| o.luminance.unwrap_or(0.0) + o.surround_luminance.unwrap_or(0.0);
    let total = min_f64(sum(a), sum(b));
    let carried = if total > 0.0 {
        2.0 * tolerances.luminance / total
    } else {
        f64::INFINITY
    };
    tolerances.contrast + carried
}

/// A probe's depth tolerance, as rung 2's T9 set it for depth across
/// adapters: 1e-5 relative, plus how far the depth moves over 1/16 of a
/// pixel (the larger slope of the two, `slope` being a whole pixel's step).
pub fn depth_tolerance(a: &Probe, b: &Probe) -> f64 {
    let depth = max_f64(f64::from(a.depth), f64::from(b.depth));
    let slope = max_f64(f64::from(a.slope), f64::from(b.slope));
    1e-5 * depth + slope / 16.0
}

/// The largest differences found, each against its tolerance: how much of
/// it was used (above 1 fails). A field nothing measured is 0.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Worst {
    pub pixels: f64,
    pub screen_box: f64,
    pub luminance: f64,
    pub contrast: f64,
    pub histogram: f64,
    pub depth: f64,
    /// Probes whose (class, id) differ, on an edge (allowed) and not.
    pub probe_keys_on_edges: u64,
    pub probe_keys_inside: u64,
}

impl Worst {
    pub fn json(&self) -> JsonValue {
        let used = |x: f64| JsonValue::F64(super::round6(x));
        JsonValue::object([
            ("contrast", used(self.contrast)),
            ("depth", used(self.depth)),
            ("histogram", used(self.histogram)),
            ("luminance", used(self.luminance)),
            ("pixels", used(self.pixels)),
            ("probe_keys_inside", JsonValue::U64(self.probe_keys_inside)),
            (
                "probe_keys_on_edges",
                JsonValue::U64(self.probe_keys_on_edges),
            ),
            ("screen_box", used(self.screen_box)),
        ])
    }
}

/// `apart` over `allowed`: 0 for none apart, infinite past a tolerance of 0.
fn ratio(apart: f64, allowed: f64) -> f64 {
    if apart == 0.0 {
        0.0
    } else if allowed > 0.0 {
        apart / allowed
    } else {
        f64::INFINITY
    }
}

/// A screen box's perimeter in pixels, 0 without one.
fn perimeter(b: Option<[u32; 4]>) -> f64 {
    b.map_or(0.0, |[x0, y0, x1, y1]| {
        2.0 * (f64::from(x1 - x0 + 1) + f64::from(y1 - y0 + 1))
    })
}

/// What [`FrameRecord::compare`] found.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Comparison {
    /// Each difference beyond its tolerance, or that must not be, with the
    /// JSON pointer of the first record's field.
    pub failures: Vec<String>,
    pub worst: Worst,
}

struct Judge<'t> {
    tol: &'t Tolerances,
    out: Comparison,
}

impl Judge<'_> {
    fn fail(&mut self, at: &str, what: String) {
        self.out.failures.push(format!("{at}: {what}"));
    }

    /// Records `used` (difference over tolerance) in `worst`, failing above 1.
    fn used(&mut self, at: &str, used: f64, worst: fn(&mut Worst) -> &mut f64, what: String) {
        let slot = worst(&mut self.out.worst);
        *slot = max_f64(*slot, used);
        if used > 1.0 {
            self.fail(at, what);
        }
    }

    /// Counts `a` and `b`, apart by at most [`Tolerances::pixels`] plus
    /// `more`.
    fn pixels(&mut self, at: &str, a: u64, b: u64, more: f64) {
        let allowed = self.tol.pixels as f64 + more;
        let used = ratio(a.abs_diff(b) as f64, allowed);
        let what = format!("pixels {a} and {b} (within {allowed})");
        self.used(at, used, |w| &mut w.pixels, what);
    }

    /// Two means (or contrasts, with `contrast`'s allowance), both there or
    /// both not.
    fn mean(&mut self, at: &str, a: Option<f64>, b: Option<f64>, contrast: Option<f64>) {
        let (Some(a), Some(b)) = (a, b) else {
            if a.is_some() != b.is_some() {
                self.fail(at, format!("{a:?} and {b:?}"));
            }
            return;
        };
        let (allowed, worst): (f64, fn(&mut Worst) -> &mut f64) = match contrast {
            Some(allowed) => (allowed, |w| &mut w.contrast),
            None => (self.tol.luminance, |w| &mut w.luminance),
        };
        let used = ratio((a - b).abs(), allowed);
        self.used(at, used, worst, format!("{a} and {b} (within {allowed})"));
    }

    fn object(&mut self, at: &str, a: &ObjectRecord, b: &ObjectRecord) {
        if (a.key, &a.name, a.world_box) != (b.key, &b.name, b.world_box) {
            let key = |o: &ObjectRecord| format!("{} {}", class_name(o.key.class), o.key.id);
            self.fail(at, format!("{} and {}: another object", key(a), key(b)));
            return;
        }
        let edges = max_f64(perimeter(a.screen_box), perimeter(b.screen_box));
        let more = self.tol.edge_pixels * edges;
        self.pixels(&format!("{at}/pixels"), a.pixels, b.pixels, more);
        if let (Some(sa), Some(sb)) = (a.screen_box, b.screen_box) {
            let apart = sa
                .iter()
                .zip(sb)
                .map(|(x, y)| x.abs_diff(y))
                .max()
                .unwrap_or(0);
            let used = ratio(f64::from(apart), f64::from(self.tol.screen_box));
            let what = format!("screen boxes {sa:?} and {sb:?}");
            self.used(
                &format!("{at}/screen_box"),
                used,
                |w| &mut w.screen_box,
                what,
            );
        }
        if a.pixels.min(b.pixels) >= self.tol.min_pixels {
            self.mean(&format!("{at}/luminance"), a.luminance, b.luminance, None);
            let around = (a.surround_luminance, b.surround_luminance);
            let at_around = format!("{at}/surround_luminance");
            self.mean(&at_around, around.0, around.1, None);
            let allowed = contrast_tolerance(self.tol, a, b);
            let at_contrast = format!("{at}/contrast");
            self.mean(&at_contrast, a.contrast, b.contrast, Some(allowed));
        }
    }

    fn probe(&mut self, at: &str, a: &Probe, b: &Probe) {
        if a.key != b.key {
            if a.edge || b.edge {
                self.out.worst.probe_keys_on_edges += 1;
            } else {
                self.out.worst.probe_keys_inside += 1;
                let key = |p: &Probe| format!("{} {}", class_name(p.key.class), p.key.id);
                self.fail(at, format!("{} and {}, off any edge", key(a), key(b)));
            }
            return;
        }
        let allowed = depth_tolerance(a, b);
        let used = ratio((f64::from(a.depth) - f64::from(b.depth)).abs(), allowed);
        let what = format!("depths {} and {} (within {allowed})", a.depth, b.depth);
        self.used(&format!("{at}/depth"), used, |w| &mut w.depth, what);
    }

    fn frame(&mut self, at: &str, a: &FrameEntry, b: &FrameEntry, pixels: u64) {
        if a.tick != b.tick || a.camera != b.camera {
            self.fail(at, "another tick or camera".into());
            return;
        }
        if a.objects.len() != b.objects.len() {
            self.fail(&format!("{at}/objects"), "another number of objects".into());
            return;
        }
        for (i, (oa, ob)) in a.objects.iter().zip(&b.objects).enumerate() {
            self.object(&format!("{at}/objects/{i}"), oa, ob);
        }
        let keys = |e: &FrameEntry| e.unlisted.iter().map(|u| u.key).collect::<Vec<_>>();
        if keys(a) != keys(b) {
            self.fail(&format!("{at}/unlisted"), "other unlisted keys".into());
        } else {
            for (i, (ua, ub)) in a.unlisted.iter().zip(&b.unlisted).enumerate() {
                let at = format!("{at}/unlisted/{i}/pixels");
                self.pixels(&at, ua.pixels, ub.pixels, 0.0);
            }
        }
        if a.invalid_info != b.invalid_info {
            self.fail(&format!("{at}/invalid_info"), "differs".into());
        }
        if let (Some(ca), Some(cb)) = (a.classes, b.classes) {
            for (code, (ra, rb)) in ca.iter().zip(cb).enumerate() {
                for (third, (x, y)) in ra.iter().zip(rb).enumerate() {
                    let more = self.tol.class_fraction * (*x).max(y) as f64;
                    self.pixels(&format!("{at}/classes/{code}/{third}"), *x, y, more);
                }
            }
        } else if a.classes != b.classes {
            self.fail(&format!("{at}/classes"), "null on one side".into());
        }
        if a.probes.len() != b.probes.len() {
            self.fail(&format!("{at}/probes"), "another number of probes".into());
        } else {
            for (i, (pa, pb)) in a.probes.iter().zip(&b.probes).enumerate() {
                self.probe(&format!("{at}/probes/{i}"), pa, pb);
            }
        }
        let at_luminance = format!("{at}/luminance");
        self.mean(&at_luminance, Some(a.luminance), Some(b.luminance), None);
        let moved: u64 = a
            .histogram
            .iter()
            .zip(b.histogram)
            .map(|(x, y)| x.abs_diff(y))
            .sum();
        let share = moved as f64 / 2.0 / pixels.max(1) as f64;
        let used = ratio(share, self.tol.histogram);
        let what = format!("{share} of the pixels change histogram bins");
        self.used(&format!("{at}/histogram"), used, |w| &mut w.histogram, what);
    }
}

impl FrameRecord {
    /// This record against `other`, of the same frames on another adapter:
    /// each difference beyond `tolerances` (or in what the CPU decides) as
    /// a failure, and the worst use of each tolerance.
    pub fn compare(&self, other: &Self, tolerances: &Tolerances) -> Comparison {
        let mut judge = Judge {
            tol: tolerances,
            out: Comparison::default(),
        };
        let shape = |r: &Self| (r.view, r.width, r.height, r.frames.len());
        if shape(self) != shape(other) {
            judge.fail("", "another view, size or number of frames".into());
            return judge.out;
        }
        let pixels = u64::from(self.width) * u64::from(self.height);
        for (i, (a, b)) in self.frames.iter().zip(&other.frames).enumerate() {
            judge.frame(&format!("/frames/{i}"), a, b, pixels);
        }
        judge.out
    }
}
