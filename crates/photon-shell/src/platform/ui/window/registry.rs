//! Stable browser-window roles and ordering for multi-window interactions.
//!
//! The primary window is the first opened. Closing it passes the role to the
//! oldest window opened from it, or failing that to the oldest window left.
//! Secondary windows close with their last tab; the primary follows the
//! setting for it.

use gpui::{AnyWindowHandle, App, Global};
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub(super) enum BrowserWindowRole {
    Primary,
    Secondary,
}

#[derive(Default)]
pub(super) struct BrowserWindowRegistry {
    entries: BTreeMap<u64, WindowEntry>,
    next_order: u64,
    primary: Option<u64>,
}

#[derive(Clone, Copy)]
struct WindowEntry {
    order: u64,
    role: BrowserWindowRole,
    /// The window this one was opened from, if any.
    opener: Option<u64>,
}

impl Global for BrowserWindowRegistry {}

impl BrowserWindowRegistry {
    pub(super) fn register(&mut self, id: u64, opener: Option<u64>) -> (BrowserWindowRole, u64) {
        self.register_from(id, opener.filter(|&opener| opener != id))
    }

    #[cfg(test)]
    fn register_id(&mut self, id: u64) -> (BrowserWindowRole, u64) {
        self.register_from(id, None)
    }

    fn register_from(&mut self, id: u64, opener: Option<u64>) -> (BrowserWindowRole, u64) {
        let role = if self.primary.is_none() {
            self.primary = Some(id);
            BrowserWindowRole::Primary
        } else {
            BrowserWindowRole::Secondary
        };
        let order = self.next_order;
        self.next_order = self.next_order.saturating_add(1);
        self.entries.insert(
            id,
            WindowEntry {
                order,
                role,
                opener: opener.filter(|opener| self.entries.contains_key(opener)),
            },
        );
        (role, order)
    }

    fn unregister(&mut self, id: u64) {
        if self.entries.remove(&id).is_none() {
            return;
        }
        if self.primary != Some(id) {
            return;
        }

        self.primary = None;
        let oldest = |opened_from: Option<u64>| {
            self.entries
                .iter()
                .filter(|(_, entry)| opened_from.is_none_or(|opener| entry.opener == Some(opener)))
                .min_by_key(|(_, entry)| entry.order)
                .map(|(&id, _)| id)
        };
        let Some(promoted_id) = oldest(Some(id)).or_else(|| oldest(None)) else {
            return;
        };
        self.primary = Some(promoted_id);
        let Some(promoted) = self.entries.get_mut(&promoted_id) else {
            return;
        };
        promoted.role = BrowserWindowRole::Primary;
    }

    fn fallback_order(&self, id: u64) -> (BrowserWindowRole, u64) {
        self.entries
            .get(&id)
            .map_or((BrowserWindowRole::Secondary, u64::MAX), |entry| {
                (entry.role, entry.order)
            })
    }

    /// Whether the window is the primary one. Unknown windows are not.
    pub(super) fn is_primary(id: u64, cx: &App) -> bool {
        cx.try_global::<Self>()
            .is_some_and(|registry| registry.primary == Some(id))
    }

    pub(super) fn unregister_window(id: u64, cx: &mut App) {
        cx.global_mut::<Self>().unregister(id);
    }

    pub(super) fn ordered_handles(cx: &App) -> Vec<AnyWindowHandle> {
        if let Some(handles) = cx.window_stack() {
            return handles;
        }

        let registry = cx.global::<Self>();
        let mut handles = cx.windows();
        handles.sort_by_key(|handle| registry.fallback_order(handle.window_id().as_u64()));
        handles
    }
}

impl super::BrowserWindow {
    /// Whether closing the window's last tab closes the window: always for
    /// secondary windows, and for the primary when the setting asks.
    pub(super) fn closes_with_last_tab(window: &gpui::Window, cx: &App) -> bool {
        !BrowserWindowRegistry::is_primary(window.window_handle().window_id().as_u64(), cx)
            || super::super::settings::Settings::get(cx).close_window_with_last_tab
    }
}

#[cfg(test)]
mod tests {
    use super::{BrowserWindowRegistry, BrowserWindowRole};

    #[test]
    fn first_window_is_primary_and_later_windows_are_secondary() {
        let mut registry = BrowserWindowRegistry::default();

        assert_eq!(registry.register_id(10), (BrowserWindowRole::Primary, 0));
        assert_eq!(registry.register_id(20), (BrowserWindowRole::Secondary, 1));
        assert!(registry.fallback_order(10) < registry.fallback_order(20));
    }

    #[test]
    fn closing_primary_promotes_the_oldest_remaining_window() {
        let mut registry = BrowserWindowRegistry::default();
        registry.register_id(10);
        registry.register_id(20);
        registry.register_id(30);

        registry.unregister(10);
        assert_eq!(registry.primary, Some(20));
        assert_eq!(
            registry.entries.get(&20).map(|entry| entry.role),
            Some(BrowserWindowRole::Primary)
        );
        assert_eq!(
            registry.entries.get(&30).map(|entry| entry.role),
            Some(BrowserWindowRole::Secondary)
        );
    }

    #[test]
    fn closing_primary_promotes_a_window_opened_from_it_first() {
        let mut registry = BrowserWindowRegistry::default();
        registry.register_from(10, None);
        registry.register_from(20, None);
        registry.register_from(30, Some(10));

        registry.unregister(10);
        assert_eq!(registry.primary, Some(30));
        registry.unregister(30);
        assert_eq!(registry.primary, Some(20));
    }

    #[test]
    fn closing_secondary_keeps_primary_unchanged() {
        let mut registry = BrowserWindowRegistry::default();
        registry.register_id(10);
        registry.register_id(20);

        registry.unregister(20);
        assert_eq!(registry.primary, Some(10));
    }

    #[test]
    fn registering_after_the_last_window_closes_starts_a_new_primary() {
        let mut registry = BrowserWindowRegistry::default();
        registry.register_id(10);
        registry.unregister(10);

        assert_eq!(registry.register_id(20), (BrowserWindowRole::Primary, 1));
    }
}
