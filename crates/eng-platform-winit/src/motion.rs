//! Raw mouse motion on macOS, from the GCMouse shim (`eng-platform-gcmouse`,
//! ADR-0021), where winit's own is accelerated and coalesced
//! (`map::RAW_MOTION`). Elsewhere the shim does not start and this does
//! nothing: raw motion there is winit's device motion.
//!
//! - **One shim per platform.** It starts with [`WinitPlatform::new`]
//!   (on the main thread, as winit's event loop is) and installs its
//!   handlers then, so its callback count runs across the platform's runs.
//! - **Focus.** The shim sums motion only while the window has focus, as
//!   the app was last told: it is told with the cursor grab, before the
//!   app hears `FocusGained` or `FocusLost`. Motion from while the window
//!   had focus is delivered before `FocusLost`; whatever comes after is
//!   dropped, and so is anything left when a run ends.
//! - **The drain.** Each `about_to_wait`, before the frame, the motion
//!   summed since the last one is one `PlatformEvent::MouseMotion`, if it
//!   moves at all. So motion events follow the wakes, not the mouse; the
//!   callback count ([`WinitPlatform::raw_motion_callbacks`]) shows the
//!   mouse's own rate.
//!
//! [`WinitPlatform::new`]: crate::WinitPlatform::new
//! [`WinitPlatform::raw_motion_callbacks`]: crate::WinitPlatform::raw_motion_callbacks

use eng_platform_api::PlatformEvent;
use eng_platform_gcmouse::{Callbacks, Motion, Shim};

/// The shim, where it runs.
#[derive(Debug, Default)]
pub(crate) struct RawMotion {
    shim: Option<Shim>,
}

impl RawMotion {
    /// Starts the shim: on macOS only, where it answers `Some`.
    pub(crate) fn start() -> Self {
        Self {
            shim: Shim::start(),
        }
    }

    /// Over a given shim: a fake, in the tests.
    #[cfg(test)]
    pub(crate) const fn on(shim: Shim) -> Self {
        Self { shim: Some(shim) }
    }

    /// Focus changed, as the app is about to be told. Drain first to keep
    /// the motion from before.
    pub(crate) fn focus(&mut self, focused: bool) {
        if let Some(shim) = &mut self.shim {
            shim.set_focused(focused);
        }
    }

    /// The motion since the last drain, as the contract's event: `None`
    /// when it moved nothing, or without the shim.
    pub(crate) fn drain(&mut self) -> Option<PlatformEvent> {
        event(self.shim.as_mut()?.drain())
    }

    /// The shim's callback count, where it runs.
    pub(crate) fn callbacks(&self) -> Option<Callbacks> {
        self.shim.as_ref().map(Shim::callbacks)
    }
}

/// The shim's motion as the contract's: counts, `dx` right and `dy`
/// towards the user. `None` if it moved nothing.
pub(crate) fn event(motion: Motion) -> Option<PlatformEvent> {
    if motion.is_still() {
        return None;
    }
    let (dx, dy) = motion.counts();
    Some(PlatformEvent::MouseMotion { dx, dy })
}

#[cfg(test)]
mod tests {
    use eng_platform_gcmouse::{Feed, Source};

    use super::*;

    /// No mice: the tests feed the shim themselves.
    struct NoMice;

    impl Source for NoMice {
        fn refresh(&mut self, _feed: &Feed) {}

        fn mice(&self) -> usize {
            0
        }
    }

    fn fake() -> (RawMotion, Feed) {
        let shim = Shim::with_source(Box::new(NoMice));
        let feed = shim.feed();
        (RawMotion::on(shim), feed)
    }

    fn moved(dx: f64, dy: f64) -> Option<PlatformEvent> {
        Some(PlatformEvent::MouseMotion { dx, dy })
    }

    #[test]
    fn motion_comes_only_with_focus() {
        let (mut raw, feed) = fake();
        feed.moved(3.0, 1.0);
        assert_eq!(raw.drain(), None, "before the first focus");
        raw.focus(true);
        feed.moved(3.0, 1.0);
        assert_eq!(raw.drain(), moved(3.0, -1.0), "GCMouse's y is up");
        feed.moved(-0.5, -2.0);
        raw.focus(false);
        assert_eq!(raw.drain(), None, "losing focus drops what was not drained");
        feed.moved(1.0, 1.0);
        assert_eq!(raw.drain(), None, "without focus");
    }

    #[test]
    fn a_drain_sums_its_callbacks_into_one_event() {
        let (mut raw, feed) = fake();
        raw.focus(true);
        for _ in 0..4 {
            feed.moved(0.25, -0.75);
        }
        assert_eq!(raw.drain(), moved(1.0, 3.0));
        assert_eq!(raw.drain(), None, "nothing since");
        feed.moved(2.0, 0.0);
        feed.moved(-2.0, 0.0);
        assert_eq!(raw.drain(), None, "motion that cancels is no event");
        let callbacks = raw.callbacks().expect("the fake shim runs");
        assert_eq!(callbacks.get(), 6, "every callback counts");
    }

    #[test]
    fn without_the_shim_there_is_nothing() {
        let mut raw = RawMotion::default();
        raw.focus(true);
        assert_eq!(raw.drain(), None);
        assert!(raw.callbacks().is_none());
    }

    #[test]
    fn the_shim_does_not_start_off_macos() {
        // On macOS it starts on the main thread only, which the test
        // harness's threads are not (unless `--test-threads=1`).
        if !cfg!(target_os = "macos") {
            assert!(RawMotion::start().callbacks().is_none());
        }
    }
}
