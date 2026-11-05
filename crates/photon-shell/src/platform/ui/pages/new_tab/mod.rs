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

use gpui::{
    Context, FocusHandle, ObjectFit, Render, Subscription, Window, div, img, prelude::*, px,
};

use super::super::history::BrowsingHistory;
use super::super::icons::{photon_logo, private_icon};
use super::super::layout::{h_stack, v_stack};
use super::super::motion::Presence;
use super::super::settings::Settings;
use super::super::{metrics, theme::page_palette as palette};
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
    feature: None,
    build: |context, cx| cx.new(|cx| NewTabPage::new(context, cx)).into(),
};

pub(in super::super) struct NewTabPage {
    context: PageContext,
    /// Whether the customise panel is open.
    customising: bool,
    /// Keeps the closed customise panel drawn while it animates away.
    customise_presence: Presence<()>,
    /// The customise button, which takes focus back as the panel closes.
    customise_focus: FocusHandle,
    /// The shortcut tile under the pointer, which offers to be removed.
    hovered: Option<usize>,
    /// The form for adding a shortcut, while it is open.
    adding: Option<ShortcutForm>,
    /// Keeps the closed form drawn while it fades away.
    adding_presence: Presence<ShortcutForm>,
    _settings_observer: Subscription,
    _history_observer: Subscription,
}

impl NewTabPage {
    fn new(context: PageContext, cx: &mut Context<Self>) -> Self {
        Self {
            context,
            customising: false,
            customise_presence: Presence::default(),
            customise_focus: cx.focus_handle(),
            hovered: None,
            adding: None,
            adding_presence: Presence::default(),
            // Follow changes made here, in settings, or in another window.
            _settings_observer: cx.observe_global::<Settings>(|_, cx| cx.notify()),
            _history_observer: cx.observe_global::<BrowsingHistory>(|_, cx| cx.notify()),
        }
    }

    fn set_customising(&mut self, customising: bool, cx: &mut Context<Self>) {
        self.customising = customising;
        cx.notify();
    }

    /// Follows the pointer over the tiles. Leaving one tile and entering the
    /// next can arrive in either order.
    fn hover_tile(&mut self, index: usize, hovered: bool, cx: &mut Context<Self>) {
        // A private window's shortcuts open sites but cannot be removed.
        if self.context.private {
            return;
        }
        if hovered || self.hovered == Some(index) {
            self.hovered = hovered.then_some(index);
            cx.notify();
        }
    }

    fn remove(&mut self, url: &str, cx: &mut Context<Self>) {
        if self.context.private {
            return;
        }
        self.hovered = None;
        Settings::update(cx, |settings| settings.new_tab.remove(url));
        super::super::show_alert(cx, "Shortcut removed");
    }
}

impl Render for NewTabPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = palette(window, cx);
        let settings = Settings::get(cx);
        let layout = settings.new_tab.clone();
        let elastic_scroll = crate::platform::ui::elastic_scroll_enabled(cx);
        let show_logo = settings.is_feature_enabled(photon_core::Feature::NewTabLogo);
        let show_shortcuts = settings.is_feature_enabled(photon_core::Feature::NewTabShortcuts);
        let shortcuts = if show_shortcuts {
            layout.shortcuts(&BrowsingHistory::most_visited(layout.history_needed(), cx))
        } else {
            Vec::new()
        };

        let mut content = v_stack()
            .text_color(gpui::rgb(palette.text_primary))
            .items_center()
            .gap(px(metrics::INTERNAL_PAGE_SECTION_GAP));
        if show_logo {
            content = content.child(
                img(photon_logo(palette.logo_tint))
                    .size(px(metrics::NEW_TAB_LOGO_SIZE))
                    .object_fit(ObjectFit::Contain),
            );
        }
        let private = self.context.private;
        if private {
            content = content.child(private_note(palette));
        }
        if show_shortcuts {
            content = content.child(self.shortcut_grid(shortcuts, palette, cx));
        }

        div()
            .id("new-tab-page")
            .relative()
            .size_full()
            .child(
                div()
                    .id("new-tab-page-scroll")
                    .size_full()
                    .flex()
                    .items_center()
                    .justify_center()
                    .overflow_y_scroll()
                    .elastic_scroll(elastic_scroll)
                    .p(px(metrics::INTERNAL_PAGE_INSET))
                    .child(content),
            )
            .when(!private, |page| {
                page.child(
                    div()
                        .absolute()
                        .right(px(metrics::INTERNAL_PAGE_GROUP_GAP))
                        .bottom(px(metrics::INTERNAL_PAGE_GROUP_GAP))
                        .child(self.customise_button(palette, cx)),
                )
            })
            .children(self.shortcut_form(super::super::theme::palette(window, cx), cx))
    }
}

/// Says that the window is private and what that keeps.
fn private_note(palette: super::super::theme::ThemeColors) -> impl IntoElement {
    v_stack()
        .id("new-tab-private-note")
        .role(gpui::Role::Note)
        .max_w(px(metrics::NEW_TAB_PRIVATE_NOTE_WIDTH))
        .items_center()
        .gap(px(metrics::MENU_ITEM_GAP))
        .child(
            h_stack()
                .items_center()
                .gap(px(metrics::MENU_ITEM_GAP * 2.0))
                .text_size(px(metrics::MENU_FONT_SIZE))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(gpui::rgb(palette.text_primary))
                .child(private_icon(
                    palette.text_primary,
                    metrics::TOOLBAR_ICON_SIZE,
                ))
                .child("Private window"),
        )
        .child(
            div()
                .text_center()
                .text_size(px(metrics::HISTORY_ROW_DETAIL_FONT_SIZE))
                .text_color(gpui::rgb(palette.text_secondary))
                .child(
                    "Photon won\u{2019}t keep the pages you visit, your searches or cookies \
                     from this window once you close it. Files you download are kept, and \
                     sites you sign in to still know it\u{2019}s you.",
                ),
        )
}
