//! Every page Photon draws itself, found by its name in a `photon://` address.

use gpui::{AnyElement, AnyView, App};
use photon_core::internal_page_name;

use super::PageContext;
#[cfg(target_os = "macos")]
use super::appkit;
use super::{downloads, history, new_tab, performance, settings};

/// What the shell needs to know about one of its pages.
pub(in super::super) struct PageDefinition {
    /// The page's name in its address, as `settings` in `photon://settings`.
    pub(in super::super) name: &'static str,
    /// The tab's title.
    pub(in super::super) title: &'static str,
    /// The tab's icon.
    pub(in super::super) icon: PageIcon,
    /// Whether the omnibox shows the page's address. A page you type from,
    /// such as the new tab page, leaves the field empty.
    pub(in super::super) shows_address: bool,
    /// Whether opening the page again switches to the tab already showing it.
    pub(in super::super) single_tab: bool,
    /// Builds the page's view.
    pub(in super::super) build: fn(PageContext, &mut App) -> AnyView,
}

/// A page's tab icon.
#[derive(Clone, Copy)]
pub(in super::super) enum PageIcon {
    /// The Photon logo, in its own colors.
    Logo,
    /// A monochrome symbol drawn in the tab's text color at the given size.
    Symbol(fn(u32, f32) -> AnyElement),
}

pub(in super::super) const DOWNLOADS: &PageDefinition = &downloads::PAGE;
pub(in super::super) const HISTORY: &PageDefinition = &history::PAGE;
pub(in super::super) const NEW_TAB: &PageDefinition = &new_tab::PAGE;
pub(in super::super) const PERFORMANCE: &PageDefinition = &performance::PAGE;
pub(in super::super) const SETTINGS: &PageDefinition = &settings::PAGE;
#[cfg(target_os = "macos")]
pub(in super::super) const APPKIT: &PageDefinition = &appkit::PAGE;

/// Every page, found by name.
#[cfg(target_os = "macos")]
const PAGES: [&PageDefinition; 6] = [NEW_TAB, DOWNLOADS, HISTORY, PERFORMANCE, SETTINGS, APPKIT];
#[cfg(not(target_os = "macos"))]
const PAGES: [&PageDefinition; 5] = [NEW_TAB, DOWNLOADS, HISTORY, PERFORMANCE, SETTINGS];

/// The page `url` names, if it is one of Photon's.
pub(in super::super) fn find_page(url: &str) -> Option<&'static PageDefinition> {
    let name = internal_page_name(url)?;
    PAGES
        .into_iter()
        .find(|page| page.name.eq_ignore_ascii_case(name))
}

impl PageDefinition {
    /// Whether `self` and `other` are the same page.
    pub(in super::super) fn is(&self, other: &PageDefinition) -> bool {
        std::ptr::eq(self, other)
    }
}
