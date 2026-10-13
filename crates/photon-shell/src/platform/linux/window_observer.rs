//! Linux window visibility and display observer placeholder.

use gpui::Window;

pub(crate) struct WindowObserver;

impl WindowObserver {
    pub(crate) fn new(_: &Window, _: impl Fn(bool, bool) + 'static) -> Option<Self> {
        None
    }
}
