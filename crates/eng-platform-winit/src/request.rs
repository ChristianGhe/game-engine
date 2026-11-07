//! The app's cursor and fullscreen requests (ADR-0022), carried out on
//! winit's window.
//!
//! - **A grab never holds without focus.** macOS's lock is the whole
//!   system's (winit stops the cursor following the mouse everywhere), so
//!   a grab held while another app has focus would take the user's cursor
//!   away from it. A grab asked for while the window lacks focus, as during
//!   `App::window` before the window shows, waits for focus and answers
//!   `None` until then; losing focus frees the cursor and drops the grab,
//!   and the app asks again when focus comes back. X11 cannot grab a
//!   window that is not mapped yet, so waiting for focus is also what makes
//!   a grab asked for in `App::window` hold there.
//! - **Grabbing** hides the cursor, moves it to the middle of the window,
//!   then tries winit's `Locked` (macOS, Windows, Wayland), then `Confined`
//!   (X11, which has no lock). macOS and Windows hold the cursor where it
//!   is, and a cursor held over another window would click into it.
//! - **Fullscreen** is winit's borderless fullscreen on the window's
//!   monitor, asked for at once. A request made during `App::window`, on a
//!   hidden window, is asked again once the window shows if it did not
//!   take: winit on macOS drops one for a window with no screen (on an M2
//!   Pro, the hidden window had one and it took). macOS animates the
//!   change, and leaving asked for just after entering began (at the first
//!   full-size `Resized`) was lost: the window stayed fullscreen while
//!   winit took it as windowed. Asked two seconds later, it worked.

use eng_platform_api::CursorGrab;
use winit::dpi::PhysicalPosition;
use winit::window::{CursorGrabMode, Fullscreen, Window};

/// What the app asked of the window, and what is in force.
#[derive(Debug, Default)]
pub(crate) struct Requests {
    /// The grab in force.
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
    pub(crate) fn grab(&mut self, window: &Window, grab: bool) -> CursorGrab {
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
    pub(crate) fn fullscreen(&mut self, window: &Window, on: bool) -> bool {
        self.fullscreen = Some(on);
        window.set_fullscreen(on.then_some(Fullscreen::Borderless(None)));
        true
    }

    /// The window has just been shown, after `App::window`: fullscreen
    /// asked for there and not taken is asked again.
    pub(crate) fn shown(&mut self, window: &Window) {
        if let Some(on) = self.fullscreen
            && window.fullscreen().is_some() != on
        {
            window.set_fullscreen(on.then_some(Fullscreen::Borderless(None)));
        }
    }

    /// The window gained focus (before the app hears of it): a grab that
    /// was waiting for it takes now.
    pub(crate) fn focus_gained(&mut self, window: &Window) {
        self.focused = true;
        if std::mem::take(&mut self.waiting) {
            self.take(window);
        }
    }

    /// The window lost focus (before the app hears of it): the cursor is
    /// freed, and the app asks again when focus comes back.
    pub(crate) fn focus_lost(&mut self, window: &Window) {
        self.focused = false;
        if self.waiting || self.grab != CursorGrab::None {
            self.free(window);
        }
    }

    /// The run is over: the cursor is freed before the window goes, since
    /// macOS's lock would outlive it.
    pub(crate) fn finish(&mut self, window: &Window) {
        if self.grab != CursorGrab::None {
            self.free(window);
        }
    }

    fn take(&mut self, window: &Window) {
        window.set_cursor_visible(false);
        // Before the lock: on macOS winit moving the cursor lets it follow
        // the mouse again. Wayland moves it only while locked; it stays.
        let size = window.inner_size();
        let middle = PhysicalPosition::new(size.width / 2, size.height / 2);
        let _ = window.set_cursor_position(middle);
        self.grab = if window.set_cursor_grab(CursorGrabMode::Locked).is_ok() {
            CursorGrab::Locked
        } else if window.set_cursor_grab(CursorGrabMode::Confined).is_ok() {
            CursorGrab::Confined
        } else {
            CursorGrab::None
        };
    }

    fn free(&mut self, window: &Window) {
        self.waiting = false;
        let _ = window.set_cursor_grab(CursorGrabMode::None);
        window.set_cursor_visible(true);
        self.grab = CursorGrab::None;
    }
}
