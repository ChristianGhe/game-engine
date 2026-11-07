//! The macOS side: GCMouse's mice, each with a moved handler that calls
//! the shim's [`Feed`]. Built only on macOS.
//!
//! The one file in the engine allowed `unsafe` code (ADR-0021). Every
//! method it needs is an `unsafe fn` in `objc2-game-controller` 0.3.2:
//! `GCMouse::mice`, `GCMouse::mouseInput` and
//! `GCMouseInput::setMouseMovedHandler`. Each call is one `unsafe` block
//! with a `// SAFETY:` comment; everything else here is safe code, and the
//! handler's body is the safe [`Feed::moved`], run by
//! [`moved_handler`]. This file declares no modules, so the exception
//! covers its text alone.
//!
//! The arguments the comments rest on:
//!
//! - **The signatures are the framework's.** The bindings are generated
//!   from Apple's headers (`GCMouse.h`, `GCMouseInput.h`) by objc2's
//!   header-translator, with the selectors, argument and return types the
//!   headers give; they are `unsafe` because the generator cannot prove
//!   any Objective-C method safe, not for a precondition the headers state.
//!   The only stated one is `setMouseMovedHandler`'s: "a valid pointer or
//!   null".
//! - **The class exists and the thread is the main one.** [`Mice::new`]
//!   checks both, with safe calls, before any binding runs: `GCMouse` is
//!   looked up by name (macOS 11 and later have it; objc2 would panic on a
//!   missing class), and [`MainThreadMarker`] proves the main thread, where
//!   winit's event loop runs and where the framework runs handlers by
//!   default. [`Mice`] keeps the marker, so it cannot leave that thread.
//! - **Objects are alive while used.** Each mouse and its input are held
//!   as `Retained`, a strong reference, for as long as a handler is
//!   installed on them.
//! - **The handler outlives every call.** It is an `RcBlock`, a
//!   reference-counted heap block: the setter copies it (the property is
//!   `copy`, which for a heap block takes a reference), so the framework
//!   holds its own reference for as long as the handler is installed, and
//!   any call it has queued holds one too. The shim keeps its own until it
//!   has set the handler back to null. The closure owns all it uses, a
//!   [`Feed`] (an `Arc` to a mutex), so it is sound to call at any time,
//!   on any thread, even after the shim is gone. `RcBlock` itself asks
//!   nothing of the closure's thread safety, so [`thread_safe`] makes the
//!   compiler check it: the closure must be `Send + Sync + 'static`, and
//!   capturing an `Rc` or a `Cell` would not compile.
//! - **The block's type is the handler's.** It is built as `GCMouseMoved`'s
//!   `dyn Fn(NonNull<GCMouseInput>, c_float, c_float)`, which the setter's
//!   argument type checks, and it ignores the first argument, so nothing
//!   depends on that pointer.
//! - **Nothing unwinds into the framework.** block2 calls the closure
//!   through an `extern "C-unwind"` trampoline, so a panic in it would
//!   unwind into GameController's and libdispatch's frames.
//!   [`moved_handler`] runs the body under `catch_unwind` and aborts the
//!   process if it panics. [`Feed::moved`] is not expected to: it takes a
//!   poisoned lock anyway and its sums saturate.
//! - **Autorelease pools.** [`Mice::refresh`](Source::refresh) and
//!   `Drop for Mice` run inside `autoreleasepool`: the first refresh runs
//!   when the platform is made, before any run loop has made a pool, and
//!   Objective-C getters may autorelease what they return.
#![allow(unsafe_code)]

use core::ffi::c_float;
use core::ptr::{self, NonNull};

use block2::RcBlock;
use objc2::MainThreadMarker;
use objc2::rc::{Retained, autoreleasepool};
use objc2::runtime::AnyClass;
use objc2_game_controller::{GCMouse, GCMouseInput};

use crate::shim::{Feed, Source};

/// A moved handler: `GCMouseMoved`'s block type, owned.
type Moved = RcBlock<dyn Fn(NonNull<GCMouseInput>, c_float, c_float)>;

/// A mouse whose input has the shim's handler.
struct Installed {
    mouse: Retained<GCMouse>,
    input: Retained<GCMouseInput>,
    /// Kept until the handler is set back to null.
    handler: Moved,
}

/// GCMouse's mice, each with the shim's moved handler.
pub(crate) struct Mice {
    installed: Vec<Installed>,
    /// Made only on the main thread, and kept there.
    _main: MainThreadMarker,
}

impl Mice {
    /// `None` off the main thread, or where the framework has no `GCMouse`
    /// class (before macOS 11).
    pub(crate) fn new() -> Option<Self> {
        let main = MainThreadMarker::new()?;
        AnyClass::get(c"GCMouse")?;
        Some(Self {
            installed: Vec::new(),
            _main: main,
        })
    }
}

impl Source for Mice {
    fn refresh(&mut self, feed: &Feed) {
        autoreleasepool(|_| self.look(feed));
    }

    fn mice(&self) -> usize {
        self.installed.len()
    }
}

impl Mice {
    /// [`Source::refresh`], inside an autorelease pool.
    fn look(&mut self, feed: &Feed) {
        // SAFETY: `+[GCMouse mice]` is a class property getter with no
        // arguments, returning a retained `NSArray<GCMouse *>` (the
        // module's first point); the class exists and this is the main
        // thread (second point: `Mice::new`, which made `self`).
        let mice = unsafe { GCMouse::mice() }.to_vec();
        // Mice that went: their handlers come off.
        self.installed.retain(|one| {
            let here = mice.iter().any(|mouse| same(mouse, &one.mouse));
            if !here {
                remove(&one.input);
            }
            here
        });
        for mouse in mice {
            if self.installed.iter().any(|one| same(&one.mouse, &mouse)) {
                continue;
            }
            // SAFETY: `-[GCMouse mouseInput]` is a property getter with no
            // arguments, returning a retained `GCMouseInput` or nil (first
            // point), called on a mouse held as `Retained` (third point).
            let Some(input) = (unsafe { mouse.mouseInput() }) else {
                // No input yet: looked for again at the next refresh.
                continue;
            };
            let handler = moved_handler(feed.clone());
            // SAFETY: the pointer is valid, as the setter requires: it is
            // `handler`'s, a live heap block that `Installed` keeps until
            // `remove` has set the handler to null, and the setter copies
            // it; the closure owns its `Feed` and the compiler has checked
            // it is `Send + Sync + 'static` (`thread_safe`; fourth point).
            // Its type is `GCMouseMoved`'s (fifth), it aborts rather than
            // unwind (sixth), and `input` is held as `Retained` (third).
            unsafe { input.setMouseMovedHandler(RcBlock::as_ptr(&handler)) };
            self.installed.push(Installed {
                mouse,
                input,
                handler,
            });
        }
    }
}

impl Drop for Mice {
    fn drop(&mut self) {
        autoreleasepool(|_| {
            for one in self.installed.drain(..) {
                remove(&one.input);
                // Only now does the shim's reference to the handler go.
                drop(one.handler);
            }
        });
    }
}

/// The moved handler for one mouse: [`Feed::moved`], aborting the process
/// rather than unwinding into the framework if it ever panics (the
/// module's sixth point).
fn moved_handler(feed: Feed) -> Moved {
    RcBlock::new(thread_safe(
        move |_: NonNull<GCMouseInput>, delta_x: c_float, delta_y: c_float| {
            if std::panic::catch_unwind(|| feed.moved(delta_x, delta_y)).is_err() {
                std::process::abort();
            }
        },
    ))
}

/// The closure, unchanged, once the compiler has checked that it may be
/// called from any thread at any time (the module's fourth point): block2's
/// `RcBlock` does not ask.
const fn thread_safe<F>(f: F) -> F
where
    F: Fn(NonNull<GCMouseInput>, c_float, c_float) + Send + Sync + 'static,
{
    f
}

/// Whether two references are to the same mouse object.
fn same(a: &Retained<GCMouse>, b: &Retained<GCMouse>) -> bool {
    ptr::eq(Retained::as_ptr(a), Retained::as_ptr(b))
}

/// Takes the shim's handler off `input`.
fn remove(input: &GCMouseInput) {
    // SAFETY: null is the other value the setter allows ("a valid pointer
    // or null"): it removes the handler, and the framework lets go of its
    // copy. `input` is alive: the caller holds it as `Retained` (third
    // point).
    unsafe { input.setMouseMovedHandler(ptr::null_mut()) };
}
