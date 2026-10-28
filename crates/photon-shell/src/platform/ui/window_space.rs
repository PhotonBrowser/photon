//! The space each window shows. Each browser window keeps its own and
//! records it here, so views that only have the window, such as its palette
//! or the settings page in it, follow that window's space. Windows without
//! one recorded show the space new windows open in.

use std::collections::HashMap;

use gpui::{App, Global, Window};
use photon_core::{Space, SpaceId};

use super::settings::Settings;

#[derive(Default)]
struct WindowSpaces(HashMap<u64, SpaceId>);

impl Global for WindowSpaces {}

/// `space` while it still exists, otherwise the space new windows open in.
pub(super) fn resolve_space(space: SpaceId, cx: &App) -> &Space {
    let spaces = &Settings::get(cx).spaces;
    spaces.get(space).unwrap_or_else(|| spaces.active())
}

/// The space the window `window_id` shows.
pub(super) fn space_of(window_id: u64, cx: &App) -> &Space {
    match cx
        .try_global::<WindowSpaces>()
        .and_then(|spaces| spaces.0.get(&window_id))
    {
        Some(&space) => resolve_space(space, cx),
        None => Settings::get(cx).spaces.active(),
    }
}

/// The space `window` shows.
pub(super) fn window_space<'a>(window: &Window, cx: &'a App) -> &'a Space {
    space_of(window.window_handle().window_id().as_u64(), cx)
}

/// Records that the window `window_id` shows `space`.
pub(super) fn set_window_space(window_id: u64, space: SpaceId, cx: &mut App) {
    cx.default_global::<WindowSpaces>()
        .0
        .insert(window_id, space);
}

/// Forgets the window `window_id` as it closes.
pub(super) fn forget_window_space(window_id: u64, cx: &mut App) {
    if let Some(spaces) = cx.try_global::<WindowSpaces>()
        && spaces.0.contains_key(&window_id)
    {
        cx.global_mut::<WindowSpaces>().0.remove(&window_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use photon_core::{BrowserSettings, WindowColor};

    #[gpui::test]
    fn windows_show_their_own_space_or_the_default(cx: &mut gpui::TestAppContext) {
        let mut settings = BrowserSettings::default();
        let first = settings.spaces.active().id;
        let second = settings.spaces.add(None, WindowColor::Blue, false);
        cx.update(|cx| {
            super::super::settings::load_settings(None, settings, None, cx);
            assert_eq!(space_of(1, cx).id, first, "unknown windows use the default");
            set_window_space(1, second, cx);
            assert_eq!(space_of(1, cx).id, second);
            assert_eq!(space_of(2, cx).id, first, "other windows are unaffected");
            set_window_space(2, 999, cx);
            assert_eq!(space_of(2, cx).id, first, "a missing space falls back");
            forget_window_space(1, cx);
            assert_eq!(space_of(1, cx).id, first);
        });
    }
}
