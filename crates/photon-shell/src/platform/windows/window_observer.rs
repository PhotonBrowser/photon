//! Windows visibility and display observer contract.

use gpui::Window;

pub(crate) struct WindowObserver;

impl WindowObserver {
    pub(crate) fn new(_: &Window, _: impl Fn(bool, bool) + 'static) -> Option<Self> {
        None
    }
}
