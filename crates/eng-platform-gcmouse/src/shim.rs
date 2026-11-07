//! The shim's safe side, the same on every target: the sum the handlers
//! feed, focus, the callback count, and when to look for mice again.
//!
//! The sum sits behind a mutex. GCMouse runs its handlers on the device's
//! `handlerQueue`, the main queue by default and the thread winit's event
//! loop drains, so the lock is never contended; it keeps the sum sound
//! whatever thread a handler runs on. [`Feed::moved`] is not expected to
//! panic: a poisoned lock is taken anyway and every sum saturates. The
//! macOS handler aborts the process if it ever does, rather than unwind
//! into the framework (`macos.rs`).
#![forbid(unsafe_code)]

use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

/// Whether GCMouse's `deltaY` is positive up, away from the user. SDL 3
/// takes it so: both its GCMouse handlers send `-deltaY` as the motion's y
/// (`src/video/cocoa/SDL_cocoamouse.m`, `Cocoa_OnGCMouseConnected`, for
/// macOS, and `src/video/uikit/SDL_uikitevents.m` for iOS; read on SDL's
/// `main` at 08ab399 on 2026-11-07). The shim follows; not yet checked on
/// a real mouse (rung 2, T14's carry). If the owner's check finds it
/// positive towards the user, this becomes `false`.
const DELTA_Y_UP: bool = true;

/// How often a focused shim looks for mice that came since it last did.
/// Mice already found need no looking: their handlers feed it.
const REFRESH: Duration = Duration::from_secs(1);

/// Where the shim's motion comes from: GCMouse's mice on macOS, or a
/// test's fake.
pub trait Source {
    /// Looks for mice, and has each one not yet found call
    /// [`Feed::moved`] on a clone of `feed` for every move. The shim calls
    /// it when it starts, when focus comes, and while focused at most once
    /// a second, from its drain. A mouse that went stops feeding it.
    fn refresh(&mut self, feed: &Feed);

    /// The mice feeding it now.
    fn mice(&self) -> usize;
}

/// What the handlers add into, between two drains.
#[derive(Debug, Default)]
struct State {
    /// The window has focus, as the shim was last told.
    focused: bool,
    /// Right, in 1/256 counts, since the last drain.
    dx: i64,
    /// Towards the user, in 1/256 counts, since the last drain.
    dy: i64,
    /// The callbacks summed into `dx` and `dy`.
    summed: u64,
    /// Every callback since the shim started, focused or not.
    total: u64,
    /// The mice the source fed from, as of its last look.
    mice: usize,
}

/// The handlers' side of the shim: where each GCMouse callback goes. A
/// [`Source`] clones it into each mouse's handler; tests call
/// [`moved`](Self::moved) themselves. Cheap to clone, and usable from any
/// thread.
#[derive(Clone, Default)]
pub struct Feed(Arc<Mutex<State>>);

impl Feed {
    /// One callback of GCMouse's `mouseMovedHandler`, with its deltas as
    /// GCMouse gives them: `delta_x` positive right, `delta_y` positive up
    /// (see the crate's "Signs and units"). It is counted; while focused
    /// its deltas are rounded to 1/256 counts and summed, with `y` turned
    /// to the contract's sign, positive towards the user. A delta that is
    /// not finite adds nothing.
    pub fn moved(&self, delta_x: f32, delta_y: f32) {
        let mut state = self.lock();
        state.total = state.total.saturating_add(1);
        if !state.focused {
            return;
        }
        let dx = sub_counts(delta_x);
        let dy = if DELTA_Y_UP {
            0_i64.saturating_sub(sub_counts(delta_y))
        } else {
            sub_counts(delta_y)
        };
        state.dx = state.dx.saturating_add(dx);
        state.dy = state.dy.saturating_add(dy);
        state.summed = state.summed.saturating_add(1);
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

impl fmt::Debug for Feed {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("Feed").field(&*self.lock()).finish()
    }
}

/// A delta in 1/256 counts, rounded to nearest, ties away from zero; 0 if
/// it is not finite. As `eng_input`'s mouse look rounds: `f32` to `f64` is
/// exact, `× 256` only changes the exponent, `round` is exact, and `as`
/// saturates.
fn sub_counts(delta: f32) -> i64 {
    let delta = f64::from(delta);
    if delta.is_finite() {
        (delta * 256.0).round() as i64
    } else {
        0
    }
}

/// The motion summed between two drains, in the contract's signs: `dx`
/// right and `dy` towards the user (down the screen), both in 1/256
/// counts, exact.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Motion {
    /// Right, in 1/256 counts.
    pub dx: i64,
    /// Towards the user, in 1/256 counts.
    pub dy: i64,
    /// The callbacks summed here.
    pub callbacks: u64,
}

impl Motion {
    /// Sub-counts per count.
    pub const SUB_COUNTS: i64 = 256;

    /// Whether it moves nothing (its callbacks may still be more than 0).
    pub const fn is_still(&self) -> bool {
        self.dx == 0 && self.dy == 0
    }

    /// `(dx, dy)` in counts, as the contract's `MouseMotion` carries them:
    /// exact while each sum is below 2^53 sub-counts, so mouse look's
    /// rounding gives back the same sub-counts.
    pub fn counts(&self) -> (f64, f64) {
        let counts = |n: i64| n as f64 / Self::SUB_COUNTS as f64;
        (counts(self.dx), counts(self.dy))
    }
}

/// A count of the shim's callbacks since it started, focused or not:
/// readable at any time, from any thread, while the shim runs and after.
#[derive(Clone, Debug)]
pub struct Callbacks(Feed);

impl Callbacks {
    /// The callbacks so far.
    pub fn get(&self) -> u64 {
        self.0.lock().total
    }

    /// The mice the shim found, as of the last time it looked. GCMouse
    /// lists a mouse once the framework has seen it, which may be after
    /// the shim starts, so it looks again ([`Source::refresh`]); with no
    /// mouse found, no callback comes.
    pub fn mice(&self) -> usize {
        self.0.lock().mice
    }
}

/// The GCMouse shim: raw mouse motion summed between drains, while the
/// window has focus, and a count of the callbacks.
pub struct Shim {
    feed: Feed,
    source: Box<dyn Source>,
    /// When the source last looked for mice.
    refreshed: Instant,
}

impl Shim {
    /// Starts the shim on GCMouse's mice: on macOS, on the main thread,
    /// where the Game Controller framework has `GCMouse` (macOS 11 and
    /// later). `None` anywhere else: the crate then does nothing. It starts
    /// without focus, so nothing is summed until
    /// [`set_focused`](Self::set_focused).
    pub fn start() -> Option<Self> {
        #[cfg(target_os = "macos")]
        {
            crate::macos::Mice::new().map(|mice| Self::with_source(Box::new(mice)))
        }
        #[cfg(not(target_os = "macos"))]
        {
            None
        }
    }

    /// The shim on another source: a fake, for tests. The source is asked
    /// to look for mice at once.
    pub fn with_source(source: Box<dyn Source>) -> Self {
        let mut shim = Self {
            feed: Feed::default(),
            source,
            refreshed: Instant::now(),
        };
        shim.refresh();
        shim
    }

    /// The feed its source's handlers call: for tests, to stand in for a
    /// mouse.
    pub fn feed(&self) -> Feed {
        self.feed.clone()
    }

    /// Whether the window has focus, as the shim was last told.
    pub fn focused(&self) -> bool {
        self.feed.lock().focused
    }

    /// Focus changed. Motion is summed only while focused; a change drops
    /// what was not drained, so drain first to keep it. Gaining focus also
    /// looks for mice that came while the window lacked it.
    pub fn set_focused(&mut self, focused: bool) {
        {
            let mut state = self.feed.lock();
            if state.focused == focused {
                return;
            }
            state.focused = focused;
            state.dx = 0;
            state.dy = 0;
            state.summed = 0;
        }
        if focused {
            self.refresh();
        }
    }

    /// The motion summed since the last drain, which starts a new sum.
    /// While focused, it also looks for new mice when a second has passed
    /// since it last did.
    pub fn drain(&mut self) -> Motion {
        if self.focused() && self.refreshed.elapsed() >= REFRESH {
            self.refresh();
        }
        let mut state = self.feed.lock();
        let motion = Motion {
            dx: state.dx,
            dy: state.dy,
            callbacks: state.summed,
        };
        state.dx = 0;
        state.dy = 0;
        state.summed = 0;
        motion
    }

    /// The count of its callbacks, from now on and for as long as anyone
    /// holds it.
    pub fn callbacks(&self) -> Callbacks {
        Callbacks(self.feed.clone())
    }

    /// The mice it found, as of the last time it looked (as
    /// [`Callbacks::mice`]).
    pub fn mice(&self) -> usize {
        self.feed.lock().mice
    }

    fn refresh(&mut self) {
        self.source.refresh(&self.feed);
        self.refreshed = Instant::now();
        let mice = self.source.mice();
        self.feed.lock().mice = mice;
    }
}

impl fmt::Debug for Shim {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Shim")
            .field("feed", &self.feed)
            .field("refreshed", &self.refreshed)
            .finish_non_exhaustive()
    }
}
