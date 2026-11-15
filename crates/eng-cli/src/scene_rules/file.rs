//! The rules file's TOML ([`super::SceneRules::parse`]), read strictly
//! through eng-schema-json's authored-file loader.

use std::collections::BTreeMap;

use eng_schema_json::{Authored, load_toml};
use serde::Deserialize;

use super::{Check, Exception, MustSee, SceneRules, ShotRules};

/// The rules file's format, its `version`.
pub const RULES_VERSION: u32 = 1;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RulesFile {
    /// Checked by `load_toml`.
    #[serde(rename = "version")]
    _version: u32,
    map: String,
    #[serde(rename = "rule-1")]
    rule_1: Rule1,
    #[serde(rename = "rule-2")]
    rule_2: Rule2,
    #[serde(rename = "rule-3")]
    rule_3: Rule3,
    shot: BTreeMap<String, Shot>,
}

impl Authored for RulesFile {
    const KIND: &'static str = "scene rules file";
    const VERSION: u32 = RULES_VERSION;
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Rule1 {
    pixels: u64,
    contrast: f64,
    light: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Rule2 {
    void: u64,
    sky: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Rule3 {
    share: f64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Shot {
    must_see: Vec<String>,
    #[serde(default)]
    except: BTreeMap<String, Except>,
}

/// `{ pixels = N, why = "..." }` or `{ share = S, why = "..." }`.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Except {
    pixels: Option<u64>,
    share: Option<f64>,
    why: String,
}

pub(super) fn parse(text: &str, origin: &str) -> Result<SceneRules, String> {
    let file: RulesFile = load_toml(text, origin).map_err(|e| e.to_string())?;
    let fail = |what: String| Err(format!("[E-LOAD] {origin}: {what}"));
    let Rule1 {
        pixels,
        contrast,
        light,
    } = file.rule_1;
    let finite_positive = |x: f64| x.is_finite() && x > 0.0;
    if pixels == 0 || !finite_positive(contrast) || !finite_positive(light) {
        return fail(format!(
            "[rule-1] pixels {pixels}, contrast {contrast} and light {light} must each be above 0"
        ));
    }
    let share = file.rule_3.share;
    if !(finite_positive(share) && share <= 1.0) {
        return fail(format!(
            "[rule-3] share {share} must be above 0 and at most 1"
        ));
    }
    if file.map.is_empty() {
        return fail("map is empty".into());
    }
    if file.shot.is_empty() {
        return fail("no [shot.<name>] table: the rules judge no shot".into());
    }
    let mut rules = SceneRules {
        map: file.map,
        must_see: MustSee {
            pixels,
            contrast,
            light,
        },
        void: file.rule_2.void,
        sky: file.rule_2.sky,
        share,
        shots: BTreeMap::new(),
    };
    for (name, shot) in file.shot {
        let at = format!("[shot.{name}]");
        let mut seen: Vec<&String> = shot.must_see.iter().collect();
        seen.sort();
        seen.dedup();
        if seen.len() != shot.must_see.len() || seen.iter().any(|n| n.is_empty()) {
            return fail(format!(
                "{at} must_see names an object twice, or by an empty name"
            ));
        }
        let mut except = BTreeMap::new();
        for (key, bounded) in shot.except {
            let at = format!("{at} except.{key}");
            let exception = exception(&rules, &key, bounded).map_err(|e| format!("{at}: {e}"));
            let (check, exception) = match exception {
                Ok(found) => found,
                Err(e) => return fail(e),
            };
            except.insert(check, exception);
        }
        let shot_rules = ShotRules {
            must_see: shot.must_see,
            except,
        };
        rules.shots.insert(name, shot_rules);
    }
    Ok(rules)
}

/// The exception `key = bounded` of a shot under `rules`: a frame-wide
/// check, its own measure's bound, looser than the scene's limit, and a
/// reason.
fn exception(rules: &SceneRules, key: &str, bounded: Except) -> Result<(Check, Exception), String> {
    let measured = Check::named(key).and_then(|c| Some((c, c.measure()?)));
    let Some((check, (measure, at_most))) = measured else {
        return Err(
            "a shot is excused from rule-2-void, rule-2-sky, rule-3-floor or rule-3-above \
             (from rule 1 by not naming the object)"
                .into(),
        );
    };
    if bounded.why.trim().is_empty() {
        return Err("gives no reason (why)".into());
    }
    let scene = rules.limit(check);
    let bound = match (at_most, bounded.pixels, bounded.share) {
        (true, Some(pixels), None) if pixels as f64 > scene => pixels as f64,
        (false, None, Some(share)) if (0.0..scene).contains(&share) => share,
        _ => {
            let way = if at_most {
                "a pixel count above"
            } else {
                "a share from 0 up to below"
            };
            return Err(format!(
                "bounds the check by `{measure}` alone, {way} the scene's {scene}"
            ));
        }
    };
    let exception = Exception {
        bound,
        why: bounded.why,
    };
    Ok((check, exception))
}
