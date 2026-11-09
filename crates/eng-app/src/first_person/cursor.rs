//! A first-person window's requests (ADR-0022 with its Amendment 1,
//! proposed; plan decision 10): borderless fullscreen unless `--windowed`,
//! and the cursor grabbed for mouse look while the window has focus.
//!
//! - **In `App::window`**, once the GPU is open: fullscreen, then the
//!   grab. winit's grab asked for there waits for focus (the window is not
//!   shown yet) and answers `None`.
//! - **On `FocusGained`** the grab is asked for again, and its answer is
//!   the grab in force: the one the report names.
//! - **On `FocusLost`** the cursor is freed. The platform has freed it
//!   already (Amendment 1), so the call changes nothing there; on a new
//!   window's first `FocusLost` it drops the grab still waiting from
//!   `App::window`, which the next `FocusGained` asks for again.
//! - **A scripted run** (`--input`) never grabs: the mouse turns nothing,
//!   and a test window should not take the cursor from the desktop it
//!   runs on. It still opens fullscreen unless `--windowed`.
//! - Fullscreen is asked for once and never toggled, so macOS's enter
//!   animation, which loses a request made during it, is never disturbed.

use eng_platform_api::{Control, CursorGrab};

/// What was asked of the window and what it granted.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct Cursor {
    /// Grab for mouse look: a live run.
    pub wants_grab: bool,
    /// Fullscreen was asked for.
    pub fullscreen_asked: bool,
    /// The platform took the fullscreen request.
    pub fullscreen: bool,
    /// The grab was asked for at least once.
    pub grab_asked: bool,
    /// The grab last granted, `None` if none ever was.
    pub granted: CursorGrab,
    /// The grab in force, as last answered.
    pub now: CursorGrab,
    /// Times the window lost focus.
    pub focus_lost: u64,
}

impl Cursor {
    /// For a live run (`wants_grab`) or a scripted one.
    pub(crate) fn new(wants_grab: bool) -> Self {
        Self {
            wants_grab,
            ..Self::default()
        }
    }

    /// The window is open: fullscreen unless `windowed`, then the grab.
    pub(crate) fn window(&mut self, ctx: &mut dyn Control, windowed: bool) {
        if !windowed {
            self.fullscreen_asked = true;
            self.fullscreen = ctx.set_fullscreen(true);
        }
        self.grab(ctx);
    }

    pub(crate) fn focus_gained(&mut self, ctx: &mut dyn Control) {
        self.grab(ctx);
    }

    pub(crate) fn focus_lost(&mut self, ctx: &mut dyn Control) {
        self.focus_lost += 1;
        if self.wants_grab {
            self.now = ctx.grab_cursor(false);
        }
    }

    fn grab(&mut self, ctx: &mut dyn Control) {
        if !self.wants_grab {
            return;
        }
        self.grab_asked = true;
        self.now = ctx.grab_cursor(true);
        if self.now != CursorGrab::None {
            self.granted = self.now;
        }
    }
}

#[cfg(test)]
mod tests {
    use eng_platform_api::{Control, CursorGrab, Instant, Request};

    use super::Cursor;

    /// A platform's controls that grant what they are told to and keep
    /// the requests.
    struct Fake {
        grants: CursorGrab,
        takes_fullscreen: bool,
        requests: Vec<Request>,
    }

    impl Control for Fake {
        fn now(&self) -> Instant {
            Instant::ZERO
        }
        fn exit(&mut self) {}
        fn grab_cursor(&mut self, grab: bool) -> CursorGrab {
            self.requests.push(Request::GrabCursor(grab));
            if grab { self.grants } else { CursorGrab::None }
        }
        fn set_fullscreen(&mut self, on: bool) -> bool {
            self.requests.push(Request::Fullscreen(on));
            self.takes_fullscreen
        }
    }

    fn fake(grants: CursorGrab) -> Fake {
        Fake {
            grants,
            takes_fullscreen: true,
            requests: Vec::new(),
        }
    }

    #[test]
    fn a_live_window_goes_fullscreen_and_grabs_on_each_focus() {
        let mut ctx = fake(CursorGrab::None);
        let mut cursor = Cursor::new(true);
        // winit: the grab waits for focus in App::window.
        cursor.window(&mut ctx, false);
        assert_eq!((cursor.fullscreen_asked, cursor.fullscreen), (true, true));
        assert!(cursor.grab_asked, "asked in App::window, waiting for focus");
        assert_eq!(
            (cursor.granted, cursor.now),
            (CursorGrab::None, CursorGrab::None)
        );
        ctx.grants = CursorGrab::Locked;
        cursor.focus_gained(&mut ctx);
        assert_eq!(
            (cursor.granted, cursor.now),
            (CursorGrab::Locked, CursorGrab::Locked)
        );
        cursor.focus_lost(&mut ctx);
        assert_eq!(
            (cursor.granted, cursor.now),
            (CursorGrab::Locked, CursorGrab::None)
        );
        cursor.focus_gained(&mut ctx);
        assert_eq!(cursor.now, CursorGrab::Locked);
        assert_eq!(cursor.focus_lost, 1);
        assert_eq!(
            ctx.requests,
            [
                Request::Fullscreen(true),
                Request::GrabCursor(true),
                Request::GrabCursor(true),
                Request::GrabCursor(false),
                Request::GrabCursor(true),
            ]
        );
    }

    #[test]
    fn windowed_asks_for_no_fullscreen_and_a_refusal_shows() {
        let mut ctx = fake(CursorGrab::Confined);
        ctx.takes_fullscreen = false;
        let mut cursor = Cursor::new(true);
        cursor.window(&mut ctx, true);
        assert!(!cursor.fullscreen_asked && !cursor.fullscreen);
        assert_eq!(cursor.granted, CursorGrab::Confined);
        let mut refused = Cursor::new(true);
        refused.window(&mut ctx, false);
        assert!(refused.fullscreen_asked && !refused.fullscreen);
    }

    #[test]
    fn a_scripted_window_never_grabs() {
        let mut ctx = fake(CursorGrab::Locked);
        let mut cursor = Cursor::new(false);
        cursor.window(&mut ctx, false);
        cursor.focus_gained(&mut ctx);
        cursor.focus_lost(&mut ctx);
        assert_eq!(ctx.requests, [Request::Fullscreen(true)]);
        assert!(!cursor.grab_asked);
        assert_eq!((cursor.granted, cursor.focus_lost), (CursorGrab::None, 1));
    }
}
