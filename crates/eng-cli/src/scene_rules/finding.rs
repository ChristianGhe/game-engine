//! A check's finding: its measures against their limits, how it came
//! out, and its message.

use super::Check;

/// One measure of a check: its value against its limit.
#[derive(Clone, Debug, PartialEq)]
pub struct Measure {
    /// `pixels`, `contrast`, `light` or `share`.
    pub name: &'static str,
    /// `None` where nothing could be measured (an object without pixels
    /// has no colour), which fails.
    pub value: Option<f64>,
    /// The limit applied: the scene's, or a shot's excused bound.
    pub limit: f64,
    /// Whether the value must be at most the limit (else at least).
    pub at_most: bool,
}

impl Measure {
    pub(super) fn passes(&self, limit: f64) -> bool {
        self.value
            .is_some_and(|v| if self.at_most { v <= limit } else { v >= limit })
    }

    pub fn ok(&self) -> bool {
        self.passes(self.limit)
    }

    fn text(&self) -> String {
        let value = self.value.map_or_else(|| "none".to_owned(), short);
        let op = match (self.at_most, self.ok()) {
            (true, true) => "<=",
            (true, false) => ">",
            (false, true) => ">=",
            (false, false) => "<",
        };
        format!("{} {value} {op} {}", self.name, short(self.limit))
    }
}

/// A number as messages give it: six decimals at most.
pub(super) fn short(x: f64) -> String {
    if x == x.trunc() && x.abs() < 1e15 {
        format!("{x:.0}")
    } else {
        let text = format!("{x:.6}");
        text.trim_end_matches('0').trim_end_matches('.').to_owned()
    }
}

/// How a check came out.
#[derive(Clone, Debug, PartialEq)]
pub enum Outcome {
    Pass,
    Fail,
    /// Past the scene's limit but within the shot's excused bound.
    Excused {
        scene_limit: f64,
        why: String,
    },
    /// Within the scene's own limit, though excused: the exception is
    /// stale, which fails.
    Stale {
        scene_limit: f64,
        why: String,
    },
}

/// One check of one frame (and for rule 1 one object).
#[derive(Clone, Debug, PartialEq)]
pub struct Finding {
    pub check: Check,
    /// The frame's tick.
    pub tick: u64,
    /// Rule 1's object, by name.
    pub object: Option<String>,
    pub measures: Vec<Measure>,
    /// What else the message says: where the values come from.
    pub note: String,
    pub outcome: Outcome,
}

impl Finding {
    /// Whether it fails the shot: past its limit, or excused but stale.
    pub fn fails(&self) -> bool {
        matches!(self.outcome, Outcome::Fail | Outcome::Stale { .. })
    }

    /// The message for `shot`: `[E-SCENE-RULE] rule-1 room-spawn marker
    /// (tick 0): light 0.04 < 0.2, ...` for a failure, untagged otherwise.
    pub fn message(&self, shot: &str) -> String {
        let tag = if self.fails() { "[E-SCENE-RULE] " } else { "" };
        let object = (self.object.as_deref()).map_or(String::new(), |o| format!(" {o}"));
        let mut measures: Vec<&Measure> = self.measures.iter().collect();
        // The failing measures first.
        measures.sort_by_key(|m| m.ok());
        let values: Vec<String> = measures.iter().map(|m| m.text()).collect();
        let mut text = format!(
            "{tag}{} {shot}{object} (tick {}): {}",
            self.check.name(),
            self.tick,
            values.join(", ")
        );
        if !self.note.is_empty() {
            text.push_str(&format!(" ({})", self.note));
        }
        match &self.outcome {
            Outcome::Pass | Outcome::Fail => {}
            Outcome::Excused { scene_limit, why } => text.push_str(&format!(
                "; excused from the scene's {}: {why}",
                short(*scene_limit)
            )),
            Outcome::Stale { scene_limit, why } => text.push_str(&format!(
                "; within the scene's own {}, yet the rules file excuses it ({why}): remove \
                 the exception",
                short(*scene_limit)
            )),
        }
        text
    }
}
