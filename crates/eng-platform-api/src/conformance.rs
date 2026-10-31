//! The conformance suite: the rules in the crate docs, checked on a
//! platform. Every implementation's tests call [`check`]; one that can be
//! scripted, like the headless platform, also calls [`check_scripted`].
//!
//! Probe apps record every callback; the rules are checked on the record
//! after `run` returns, so a platform that catches panics in callbacks
//! still fails cleanly. The one failure raised inside a callback is a
//! platform that keeps calling after the app's exit, which would otherwise
//! never end the run. Probes also give up after a bounded number of frames.
//! A broken rule panics with `platform conformance: <platform> (<run>):
//! <rule>: <what happened>`.

mod probe;
mod scripts;

use probe::{Call, Plan, Probe, Seen};

use crate::{Instant, Platform, PlatformError, PlatformEvent, Wake};

/// The rule a platform breaks by calling the app after its exit.
const AFTER_EXIT: &str = "nothing comes after exit but stop";

/// Fails the suite.
fn fail(platform: &str, run: &str, rule: &str, what: &str) -> ! {
    panic!("platform conformance: {platform} ({run}): {rule}: {what}")
}

/// Checks the rules every platform follows, in three runs of `platform`
/// that need no input: start and stop, the clock, timed wakes (one asked
/// for in the past), exit from a frame and from `start`, and running
/// again. Panics with the first rule broken.
pub fn check<P: Platform + ?Sized>(platform: &mut P) {
    let runs = [
        ("timed wakes", Plan::Clock),
        ("exit in start", Plan::ExitInStart),
        ("timed wakes again", Plan::Clock),
    ];
    for (run, plan) in runs {
        let mut probe = Probe::new(plan, platform.name(), run);
        let result = platform.run(&mut probe);
        let report = Report {
            platform: platform.name(),
            run,
        };
        report.lifecycle(&probe, &result);
        if plan == Plan::Clock {
            report.timed_wakes(&probe);
        }
        report.lasted(&probe, &result);
    }
}

/// Checks event delivery on a platform that can be scripted: `make` builds
/// one that delivers exactly the given events, each when its clock reaches
/// the event's time. Every event kind must arrive intact, in order and at
/// its time; every event due comes before the frame, and each batch is
/// followed by one frame; wakes come exactly when due; a close request
/// reaches the app and does not end the run by itself; and an exit drops
/// the rest. Panics with the first rule broken.
pub fn check_scripted<P: Platform>(mut make: impl FnMut(&[(Instant, PlatformEvent)]) -> P) {
    let runs = [
        ("timers and close requests", Plan::Timers, scripts::timers()),
        ("waiting for events", Plan::Events, scripts::events()),
    ];
    for (run, plan, script) in runs {
        let mut platform = make(&script);
        let mut probe = Probe::new(plan, platform.name(), run);
        let result = platform.run(&mut probe);
        let report = Report {
            platform: platform.name(),
            run,
        };
        report.lifecycle(&probe, &result);
        report.delivery(&probe, &result, &script);
    }
}

/// Names the platform and the run in every failure.
struct Report {
    platform: &'static str,
    run: &'static str,
}

impl Report {
    fn fail(&self, rule: &str, what: &str) -> ! {
        fail(self.platform, self.run, rule, what)
    }

    /// The rules about order, the clock and exit, for every run.
    fn lifecycle(&self, probe: &Probe, result: &Result<(), PlatformError>) {
        let seen = &probe.seen;
        let count = |call: Call| seen.iter().filter(|s| s.call == call).count();
        if seen.first().map(|s| s.call) != Some(Call::Start) || count(Call::Start) != 1 {
            let what = format!("the callbacks were {}; run returned {result:?}", list(seen));
            self.fail("start comes first, once", &what);
        }
        let windows: Vec<_> = (0..seen.len())
            .filter(|&i| seen[i].call == Call::Window)
            .collect();
        if windows.len() > 1 || windows.first().is_some_and(|&i| i != 1) {
            let what = format!("the callbacks were {}", list(seen));
            self.fail("the window comes once, right after start", &what);
        }
        if seen.last().map(|s| s.call) != Some(Call::Stop) || count(Call::Stop) != 1 {
            let what = format!("the callbacks were {}", list(seen));
            self.fail("stop comes last, once", &what);
        }
        if let Some(w) = seen.windows(2).find(|w| w[1].now < w[0].now) {
            let what = format!("{} came after {}", show(&w[1]), show(&w[0]));
            self.fail("the clock never goes back", &what);
        }
        if let Some(late) = seen.iter().find(|s| s.after_exit && s.call != Call::Stop) {
            let what = format!("{} came after the app exited", show(late));
            self.fail(AFTER_EXIT, &what);
        }
        if probe.exited_in.is_some()
            && let Err(error) = result
        {
            let what = format!("it returned Err({error:?})");
            self.fail("run returns Ok after the app exits", &what);
        }
    }

    /// The run lasted until the app exited. Checked after the rules that
    /// explain an early end better.
    fn lasted(&self, probe: &Probe, result: &Result<(), PlatformError>) {
        if probe.exited_in.is_none() {
            let what = format!("it returned {result:?} after {}", list(&probe.seen));
            self.fail("the run lasts until the app exits", &what);
        }
    }

    /// The clock run: a timed wake never comes early.
    fn timed_wakes(&self, probe: &Probe) {
        let mut asked = None;
        for seen in &probe.seen {
            match seen.call {
                Call::Event(_) => asked = None,
                Call::Frame(wake) => {
                    if let Some(at) = asked
                        && seen.now < at
                    {
                        let what = format!("{} came before {at}, with no event since", show(seen));
                        self.fail("a timed wake is never early", &what);
                    }
                    asked = match wake {
                        Wake::At(at) => Some(at),
                        Wake::OnEvent => None,
                    };
                }
                _ => {}
            }
        }
    }

    /// The scripted runs: events, batches, wakes and close requests.
    fn delivery(
        &self,
        probe: &Probe,
        result: &Result<(), PlatformError>,
        script: &[(Instant, PlatformEvent)],
    ) {
        let seen = &probe.seen;
        let events: Vec<&Seen> = seen
            .iter()
            .filter(|s| matches!(s.call, Call::Event(_)))
            .collect();
        let order = "every scripted event arrives once, in order";
        for (i, got) in events.iter().enumerate() {
            match script.get(i) {
                Some(&(_, want)) if got.call == Call::Event(want) => {}
                Some(&(at, want)) => {
                    let what = format!("event {} was {}, not {want:?} at {at}", i + 1, show(got));
                    self.fail(order, &what);
                }
                None => self.fail(order, &format!("{} is not in the script", show(got))),
            }
        }
        if let Some(i) = probe.ignored_close
            && seen[i + 1..].iter().all(|s| s.call == Call::Stop)
        {
            let what = format!(
                "the run ended after the close request at {}, which the app ignored",
                seen[i].now
            );
            self.fail("a close request is the app's to handle", &what);
        }
        self.lasted(probe, result);
        let exit = seen[probe.exited_in.unwrap_or(0)];
        if matches!(exit.call, Call::Frame(_))
            && let Some(&(at, missing)) = script.get(events.len()).filter(|e| e.0 <= exit.now)
        {
            let what = format!(
                "the app exited at {} without {missing:?}, due at {at}",
                exit.now
            );
            self.fail(order, &what);
        }
        for (got, &(at, _)) in events.iter().zip(script) {
            if got.now != at {
                let what = format!("{}, due at {at}", show(got));
                self.fail("an event arrives at its time", &what);
            }
        }
        self.batches(probe);
        self.wakes(probe, script);
    }

    /// Each batch of events is followed by one frame at the same time,
    /// unless the app exited during it.
    fn batches(&self, probe: &Probe) {
        let seen = &probe.seen;
        let mut i = 0;
        while i < seen.len() {
            if !matches!(seen[i].call, Call::Event(_)) {
                i += 1;
                continue;
            }
            let start = i;
            while seen
                .get(i)
                .is_some_and(|s| matches!(s.call, Call::Event(_)))
            {
                i += 1;
            }
            let exited = probe.exited_in.is_some_and(|e| (start..i).contains(&e));
            // `lifecycle` has checked that the last callback is `stop`.
            let next = &seen[i.min(seen.len() - 1)];
            if !exited && !(matches!(next.call, Call::Frame(_)) && next.now == seen[i - 1].now) {
                let what = format!(
                    "the events at {} were followed by {}",
                    seen[i - 1].now,
                    show(next)
                );
                self.fail("a frame follows each batch of events", &what);
            }
        }
    }

    /// Every event due comes before the frame, and each frame comes exactly
    /// when due: the first at once, then at the earlier of the time asked
    /// for and the next event's time.
    fn wakes(&self, probe: &Probe, script: &[(Instant, PlatformEvent)]) {
        let seen = &probe.seen;
        let mut delivered = 0;
        // The previous frame: its time, its answer, the events before it.
        let mut last: Option<(Instant, Wake, usize)> = None;
        for s in seen {
            match s.call {
                Call::Event(_) => delivered += 1,
                Call::Frame(wake) => {
                    if let Some(&(at, event)) = script.get(delivered)
                        && at <= s.now
                    {
                        let what = format!("{event:?}, due at {at}, had not come by {}", show(s));
                        self.fail("every event due comes before the frame", &what);
                    }
                    let (due, rule) = match last {
                        None => (Some(seen[0].now), "the first frame comes at once"),
                        Some((then, asked, before)) => {
                            let event = script.get(before).map(|e| e.0);
                            match asked {
                                Wake::At(at) => {
                                    let timer = at.max(then);
                                    let due = event.map_or(timer, |e| e.min(timer));
                                    (Some(due), "a timed wake comes on time")
                                }
                                Wake::OnEvent => {
                                    (event, "a wait for events ends only with an event")
                                }
                            }
                        }
                    };
                    if due != Some(s.now) {
                        let due = due.map_or("never: no event was left".into(), |t| t.to_string());
                        let asked = last
                            .map_or_else(String::new, |(t, w, _)| format!(", after {w:?} at {t}"));
                        let what = format!("{} was due at {due}{asked}", show(s));
                        self.fail(rule, &what);
                    }
                    last = Some((s.now, wake, delivered));
                }
                _ => {}
            }
        }
    }
}

fn show(seen: &Seen) -> String {
    format!("{:?} at {}", seen.call, seen.now)
}

/// The first callbacks, for messages.
fn list(seen: &[Seen]) -> String {
    if seen.is_empty() {
        return "none".to_string();
    }
    let mut shown: Vec<String> = seen.iter().take(12).map(show).collect();
    if seen.len() > 12 {
        shown.push(format!("and {} more", seen.len() - 12));
    }
    shown.join(", ")
}
