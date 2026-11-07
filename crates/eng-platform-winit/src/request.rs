//! The app's cursor and fullscreen requests (ADR-0022, with the rules its
//! Amendment 1 proposes), carried out on winit's window.
//!
//! - **A grab never holds without focus.** macOS's lock is the whole
//!   system's (winit stops the cursor following the mouse everywhere), so
//!   a grab held while another app has focus would take the user's cursor
//!   away from it. Losing focus frees the cursor before the app hears of
//!   it, and the app asks again when focus comes back.
//! - **A grab asked for without focus waits for it**, answering `None`
//!   until then, and is taken when focus comes; freeing drops it. That is
//!   the case in `App::window`, before the window shows: X11 cannot grab a
//!   window that is not mapped yet. winit reports a `FocusLost` before a
//!   new window's first `FocusGained` (seen on macOS 26 and under Xvfb),
//!   so a loss while the window already lacks focus leaves a waiting grab
//!   waiting.
//! - **Grabbing** hides the cursor, moves it to the middle of the window,
//!   then tries winit's `Locked` (macOS, Windows, Wayland), then `Confined`
//!   (X11, which has no lock). macOS and Windows hold the cursor where it
//!   is, and a cursor held over another window would click into it. If
//!   neither takes, the cursor shows again and the answer is `None`.
//! - **Fullscreen** is winit's borderless fullscreen on the window's
//!   monitor, asked for at once. A request made during `App::window`, on a
//!   hidden window, is asked again once the window shows if it did not
//!   take: winit on macOS drops one for a window with no screen (on an M2
//!   Pro, the hidden window had one and it took). macOS animates the
//!   change, and leaving asked for just after entering began (at the first
//!   full-size `Resized`) was lost: the window stayed fullscreen while
//!   winit took it as windowed. Asked two seconds later, it worked.
//!
//! [`Requests`] reaches the window only through [`Target`], so the tests
//! below check these rules without a display.

use eng_platform_api::CursorGrab;
use winit::dpi::PhysicalPosition;
use winit::window::{CursorGrabMode, Fullscreen, Window};

/// What [`Requests`] does to a window: winit's in a run, a fake in the
/// tests.
pub(crate) trait Target {
    /// Shows (`true`) or hides the cursor over the window.
    fn show_cursor(&self, shown: bool);
    /// Moves the cursor to the middle of the window, where it can.
    fn centre_cursor(&self);
    /// Sets winit's grab mode, and says whether it took.
    fn grab_cursor(&self, mode: CursorGrabMode) -> bool;
    /// Borderless fullscreen on the window's monitor (`true`), or a window.
    fn go_fullscreen(&self, on: bool);
    /// Whether the window is fullscreen now.
    fn is_fullscreen(&self) -> bool;
}

impl Target for Window {
    fn show_cursor(&self, shown: bool) {
        self.set_cursor_visible(shown);
    }

    fn centre_cursor(&self) {
        let size = self.inner_size();
        let middle = PhysicalPosition::new(size.width / 2, size.height / 2);
        let _ = self.set_cursor_position(middle);
    }

    fn grab_cursor(&self, mode: CursorGrabMode) -> bool {
        self.set_cursor_grab(mode).is_ok()
    }

    fn go_fullscreen(&self, on: bool) {
        self.set_fullscreen(on.then_some(Fullscreen::Borderless(None)));
    }

    fn is_fullscreen(&self) -> bool {
        self.fullscreen().is_some()
    }
}

/// What the app asked of the window, and what is in force.
#[derive(Debug, Default)]
pub(crate) struct Requests {
    /// The grab in force. Not `None` only while focused, with the cursor
    /// hidden; `None` with the cursor shown.
    grab: CursorGrab,
    /// The app asked for a grab while the window lacked focus.
    waiting: bool,
    /// The window has focus, as the app was last told.
    focused: bool,
    /// The fullscreen the app last asked for, if any.
    fullscreen: Option<bool>,
}

impl Requests {
    /// `Control::grab_cursor`.
    pub(crate) fn grab(&mut self, window: &impl Target, grab: bool) -> CursorGrab {
        if !grab {
            self.free(window);
        } else if self.focused {
            self.take(window);
        } else {
            self.waiting = true;
        }
        self.grab
    }

    /// The window has focus, as the app was last told.
    pub(crate) const fn focused(&self) -> bool {
        self.focused
    }

    /// `Control::set_fullscreen`: taken whenever there is a window.
    pub(crate) fn fullscreen(&mut self, window: &impl Target, on: bool) -> bool {
        self.fullscreen = Some(on);
        window.go_fullscreen(on);
        true
    }

    /// The window has just been shown, after `App::window`: fullscreen
    /// asked for there and not taken is asked again.
    pub(crate) fn shown(&mut self, window: &impl Target) {
        if let Some(on) = self.fullscreen
            && window.is_fullscreen() != on
        {
            window.go_fullscreen(on);
        }
    }

    /// The window gained focus (before the app hears of it): a grab that
    /// was waiting for it takes now.
    pub(crate) fn focus_gained(&mut self, window: &impl Target) {
        self.focused = true;
        if std::mem::take(&mut self.waiting) {
            self.take(window);
        }
    }

    /// The window lost focus (before the app hears of it): a grab in force
    /// is freed, and the app asks again when focus comes back. A grab
    /// still waiting was asked for without focus, so it keeps waiting.
    pub(crate) fn focus_lost(&mut self, window: &impl Target) {
        self.focused = false;
        if self.grab != CursorGrab::None {
            self.free(window);
        }
    }

    /// The run is over: the cursor is freed before the window goes, since
    /// macOS's lock would outlive it.
    pub(crate) fn finish(&mut self, window: &impl Target) {
        if self.grab != CursorGrab::None {
            self.free(window);
        }
    }

    fn take(&mut self, window: &impl Target) {
        window.show_cursor(false);
        // Before the lock: on macOS winit moving the cursor lets it follow
        // the mouse again. Wayland moves it only while locked; it stays.
        window.centre_cursor();
        self.grab = if window.grab_cursor(CursorGrabMode::Locked) {
            CursorGrab::Locked
        } else if window.grab_cursor(CursorGrabMode::Confined) {
            CursorGrab::Confined
        } else {
            CursorGrab::None
        };
        if self.grab == CursorGrab::None {
            // Nothing holds the cursor, so it shows, as when freed.
            window.show_cursor(true);
        }
    }

    fn free(&mut self, window: &impl Target) {
        self.waiting = false;
        let _ = window.grab_cursor(CursorGrabMode::None);
        window.show_cursor(true);
        self.grab = CursorGrab::None;
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;

    /// A window that keeps what was done to it.
    struct Fake {
        /// The grab modes that take on it.
        takes: &'static [CursorGrabMode],
        mode: Cell<CursorGrabMode>,
        shown: Cell<bool>,
        centred: Cell<u32>,
        /// Whether a fullscreen request lands (winit on macOS drops one
        /// for a window with no screen).
        lands: Cell<bool>,
        fullscreen: Cell<bool>,
        fullscreen_asked: Cell<u32>,
    }

    impl Fake {
        const fn new(takes: &'static [CursorGrabMode]) -> Self {
            Self {
                takes,
                mode: Cell::new(CursorGrabMode::None),
                shown: Cell::new(true),
                centred: Cell::new(0),
                lands: Cell::new(true),
                fullscreen: Cell::new(false),
                fullscreen_asked: Cell::new(0),
            }
        }

        /// The cursor is free and shows.
        fn free(&self) -> bool {
            self.mode.get() == CursorGrabMode::None && self.shown.get()
        }

        /// The cursor is held in `mode` and hidden.
        fn held(&self, mode: CursorGrabMode) -> bool {
            self.mode.get() == mode && !self.shown.get()
        }
    }

    impl Target for Fake {
        fn show_cursor(&self, shown: bool) {
            self.shown.set(shown);
        }

        fn centre_cursor(&self) {
            self.centred.set(self.centred.get() + 1);
        }

        fn grab_cursor(&self, mode: CursorGrabMode) -> bool {
            let took = mode == CursorGrabMode::None || self.takes.contains(&mode);
            if took {
                self.mode.set(mode);
            }
            took
        }

        fn go_fullscreen(&self, on: bool) {
            self.fullscreen_asked.set(self.fullscreen_asked.get() + 1);
            if self.lands.get() {
                self.fullscreen.set(on);
            }
        }

        fn is_fullscreen(&self) -> bool {
            self.fullscreen.get()
        }
    }

    /// macOS, Windows and Wayland: both modes.
    const LOCKS: &[CursorGrabMode] = &[CursorGrabMode::Locked, CursorGrabMode::Confined];
    /// X11: no lock.
    const CONFINES: &[CursorGrabMode] = &[CursorGrabMode::Confined];
    const NEITHER: &[CursorGrabMode] = &[];

    /// Requests on a window that has focus.
    fn focused_on(window: &Fake) -> Requests {
        let mut requests = Requests::default();
        requests.focus_gained(window);
        requests
    }

    #[test]
    fn a_grab_with_focus_locks_or_falls_back_to_confined() {
        let cases = [
            (LOCKS, CursorGrab::Locked, CursorGrabMode::Locked),
            (CONFINES, CursorGrab::Confined, CursorGrabMode::Confined),
        ];
        for (takes, answer, mode) in cases {
            let window = Fake::new(takes);
            let mut requests = focused_on(&window);
            assert_eq!(requests.grab(&window, true), answer);
            assert!(window.held(mode), "{mode:?}: held, the cursor hidden");
            assert_eq!(window.centred.get(), 1, "in the middle of the window");
            assert_eq!(requests.grab(&window, true), answer, "asked again");
            assert_eq!(requests.grab(&window, false), CursorGrab::None);
            assert!(window.free(), "{mode:?}: freeing shows the cursor");
        }
    }

    #[test]
    fn a_grab_no_mode_takes_answers_none_with_the_cursor_shown() {
        let window = Fake::new(NEITHER);
        let mut requests = focused_on(&window);
        assert_eq!(requests.grab(&window, true), CursorGrab::None);
        assert!(window.free(), "not left hidden");
        requests.focus_lost(&window);
        requests.finish(&window);
        assert!(window.free());

        // Waiting for focus, then failing, is the same.
        let window = Fake::new(NEITHER);
        let mut requests = Requests::default();
        assert_eq!(requests.grab(&window, true), CursorGrab::None);
        requests.focus_gained(&window);
        assert!(window.free());
        assert_eq!(requests.grab(&window, true), CursorGrab::None);
    }

    #[test]
    fn a_grab_asked_for_without_focus_waits_and_takes_with_it() {
        let window = Fake::new(LOCKS);
        let mut requests = Requests::default();
        assert!(!requests.focused(), "a new window lacks focus");
        // In `App::window`, the window hidden.
        assert_eq!(requests.grab(&window, true), CursorGrab::None);
        assert!(window.free(), "nothing held before focus");
        // winit's loss before a new window's first gain: still waiting.
        requests.focus_lost(&window);
        assert!(window.free());
        requests.focus_gained(&window);
        assert!(
            window.held(CursorGrabMode::Locked),
            "taken with focus, before the app hears of it"
        );
        assert_eq!(requests.grab(&window, true), CursorGrab::Locked);
    }

    #[test]
    fn freeing_drops_a_waiting_grab() {
        let window = Fake::new(LOCKS);
        let mut requests = Requests::default();
        assert_eq!(requests.grab(&window, true), CursorGrab::None);
        assert_eq!(requests.grab(&window, false), CursorGrab::None);
        requests.focus_gained(&window);
        assert!(window.free());
        assert_eq!(window.centred.get(), 0, "never grabbed");
    }

    #[test]
    fn focus_loss_frees_a_grab_and_focus_does_not_take_it_again_unasked() {
        for asked_with_focus in [true, false] {
            let window = Fake::new(LOCKS);
            let mut requests = Requests::default();
            if asked_with_focus {
                requests.focus_gained(&window);
                requests.grab(&window, true);
            } else {
                requests.grab(&window, true);
                requests.focus_gained(&window);
            }
            assert!(window.held(CursorGrabMode::Locked));
            requests.focus_lost(&window);
            assert!(window.free(), "freed and shown before FocusLost");
            assert!(!requests.focused());
            requests.focus_gained(&window);
            assert!(window.free(), "not grabbed again unasked");
            assert_eq!(requests.grab(&window, true), CursorGrab::Locked);
        }
    }

    #[test]
    fn focus_follows_the_events_the_app_is_told() {
        let window = Fake::new(LOCKS);
        let mut requests = Requests::default();
        assert!(!requests.focused());
        requests.focus_gained(&window);
        assert!(requests.focused());
        requests.focus_lost(&window);
        assert!(!requests.focused());
    }

    #[test]
    fn the_end_of_a_run_frees_a_grab_in_force() {
        let window = Fake::new(CONFINES);
        let mut requests = focused_on(&window);
        assert_eq!(requests.grab(&window, true), CursorGrab::Confined);
        requests.finish(&window);
        assert!(window.free());
    }

    #[test]
    fn fullscreen_not_taken_in_window_is_asked_again_once_shown() {
        let window = Fake::new(LOCKS);
        window.lands.set(false);
        let mut requests = Requests::default();
        assert!(
            requests.fullscreen(&window, true),
            "taken: it may land later"
        );
        assert!(!window.is_fullscreen());
        window.lands.set(true);
        requests.shown(&window);
        assert!(window.is_fullscreen(), "asked again once shown");
        assert_eq!(window.fullscreen_asked.get(), 2);

        // One that took is not asked again; leaving is asked at once.
        let window = Fake::new(LOCKS);
        let mut requests = Requests::default();
        assert!(requests.fullscreen(&window, true));
        requests.shown(&window);
        assert_eq!(window.fullscreen_asked.get(), 1);
        assert!(requests.fullscreen(&window, false));
        assert!(!window.is_fullscreen());

        // Nothing asked, nothing asked again.
        let window = Fake::new(LOCKS);
        Requests::default().shown(&window);
        assert_eq!(window.fullscreen_asked.get(), 0);
    }
}
