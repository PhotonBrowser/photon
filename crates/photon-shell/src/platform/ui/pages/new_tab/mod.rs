//! The new tab page: the Photon logo and shortcuts to sites, laid out as
//! chosen in its customise panel.
//!
//! - `tiles`: the shortcut grid.
//! - `customise`: the customise button and panel.
//! - `options`: the layout options, also shown in settings.
//! - `shortcut_form`: adding a shortcut.

mod customise;
mod options;
mod shortcut_form;
mod tiles;

use gpui::{Context, ObjectFit, Render, Subscription, Window, div, img, prelude::*, px};

use super::super::history::BrowsingHistory;
use super::super::icons::photon_logo;
use super::super::layout::v_stack;
use super::super::motion::Presence;
use super::super::settings::Settings;
use super::super::{metrics, theme::palette};
use super::{PageContext, PageDefinition, PageIcon};
pub(super) use options::LayoutOptions;
use shortcut_form::ShortcutForm;

/// The new tab page, which leaves the omnibox empty for typing.
pub(super) const PAGE: PageDefinition = PageDefinition {
    name: "newtab",
    title: "New Tab",
    icon: PageIcon::Logo,
    shows_address: false,
    single_tab: false,
    build: |context, cx| cx.new(|cx| NewTabPage::new(context, cx)).into(),
};

pub(in super::super) struct NewTabPage {
    context: PageContext,
    /// Whether the customise panel is open.
    customising: bool,
    /// Keeps the closed customise panel drawn while it animates away.
    customise_presence: Presence<()>,
    /// The shortcut tile under the pointer, which offers to be removed.
    hovered: Option<usize>,
    /// The form for adding a shortcut, while it is open.
    adding: Option<ShortcutForm>,
    _settings_observer: Subscription,
}

impl NewTabPage {
    fn new(context: PageContext, cx: &mut Context<Self>) -> Self {
        Self {
            context,
            customising: false,
            customise_presence: Presence::default(),
            hovered: None,
            adding: None,
            // Follow changes made here, in settings, or in another window.
            _settings_observer: cx.observe_global::<Settings>(|_, cx| cx.notify()),
        }
    }

    fn toggle_customising(&mut self, cx: &mut Context<Self>) {
        self.customising = !self.customising;
        cx.notify();
    }

    /// Follows the pointer over the tiles. Leaving one tile and entering the
    /// next can arrive in either order.
    fn hover_tile(&mut self, index: usize, hovered: bool, cx: &mut Context<Self>) {
        if hovered || self.hovered == Some(index) {
            self.hovered = hovered.then_some(index);
            cx.notify();
        }
    }

    fn remove(&mut self, url: &str, cx: &mut Context<Self>) {
        self.hovered = None;
        Settings::update(cx, |settings| settings.new_tab.remove(url));
    }
}

impl Render for NewTabPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = palette(window, cx);
        let layout = Settings::get(cx).new_tab.clone();
        let shortcuts =
            layout.shortcuts(&BrowsingHistory::most_visited(layout.history_needed(), cx));

        let mut content = v_stack()
            .items_center()
            .gap(px(metrics::INTERNAL_PAGE_SECTION_GAP));
        if layout.show_logo {
            content = content.child(
                img(photon_logo())
                    .size(px(metrics::NEW_TAB_LOGO_SIZE))
                    .object_fit(ObjectFit::Contain),
            );
        }
        if layout.show_shortcuts {
            content = content.child(self.shortcut_grid(shortcuts, palette, cx));
        }

        div()
            .id("new-tab-page")
            .relative()
            .size_full()
            .flex()
            .items_center()
            .justify_center()
            .p(px(metrics::INTERNAL_PAGE_INSET))
            .child(content)
            .child(
                div()
                    .absolute()
                    .right(px(metrics::INTERNAL_PAGE_GROUP_GAP))
                    .bottom(px(metrics::INTERNAL_PAGE_GROUP_GAP))
                    .child(self.customise_button(palette, cx)),
            )
            .children(self.shortcut_form(palette, cx))
    }
}
