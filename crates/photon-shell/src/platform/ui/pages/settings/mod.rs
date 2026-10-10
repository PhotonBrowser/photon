//! The settings page: a sidebar of sections and the chosen section's
//! settings.
//!
//! Each section lives in its own module and draws from the shared
//! [`Settings`]; only clearing browsing data keeps state of its own.
//! `reset` returns every setting to its default, after asking.

mod appearance;
mod new_tab;
mod privacy;
mod reset;
mod search;
mod sites;
mod tabs;

use super::super::icons::{
    appearance_icon, globe_icon, grid_icon, search_icon_sized, settings_icon, shield_icon,
    sidebar_icon,
};
use super::super::layout::{h_stack, v_stack};
use super::super::motion::{AnimateIn, Entrance, Presence};
use super::super::settings::Settings;
use super::super::{metrics, theme::ThemeColors, theme::palette};
use super::layout::{heading, scrolling_column, secondary_text};
use super::{PageContext, PageDefinition, PageIcon};
use gpui::{
    AnyElement, Context, ElementId, FocusHandle, FontWeight, MouseButton, Render, Role,
    Subscription, Window, div, prelude::*, px, rgb, rgba,
};
use privacy::ClearForm;
use std::time::Instant;

/// How a picked section appears.
const SECTION_MOTION: Entrance = Entrance::fade();

/// A part of the settings, shown on its own.
#[derive(Clone, Copy, PartialEq)]
enum Section {
    Appearance,
    Search,
    NewTab,
    Tabs,
    Sites,
    Privacy,
}

impl Section {
    const ALL: [Self; 6] = [
        Self::Appearance,
        Self::Search,
        Self::NewTab,
        Self::Tabs,
        Self::Sites,
        Self::Privacy,
    ];

    fn id(self) -> &'static str {
        match self {
            Self::Appearance => "appearance",
            Self::Search => "search",
            Self::NewTab => "new-tab",
            Self::Tabs => "tabs",
            Self::Sites => "sites",
            Self::Privacy => "privacy",
        }
    }

    fn title(self) -> &'static str {
        match self {
            Self::Appearance => "Appearance",
            Self::Search => "Search",
            Self::NewTab => "New tab page",
            Self::Tabs => "Tabs",
            Self::Sites => "Sites",
            Self::Privacy => "Privacy",
        }
    }

    fn description(self) -> String {
        let name = photon_brand::NAME;
        match self {
            Self::Appearance => format!("How {name} looks."),
            Self::Search => "Where searches from the address bar go.".to_owned(),
            Self::NewTab => "What new tabs show.".to_owned(),
            Self::Tabs => "How tabs open and close.".to_owned(),
            Self::Sites => "What sites may do.".to_owned(),
            Self::Privacy => format!("Delete what {name} and websites keep on this Mac."),
        }
    }

    fn icon(self, color: u32) -> AnyElement {
        let size = metrics::SETTINGS_SIDEBAR_ICON_SIZE;
        match self {
            Self::Appearance => appearance_icon(color, size).into_any_element(),
            Self::Search => search_icon_sized(color, size).into_any_element(),
            Self::NewTab => grid_icon(color, size).into_any_element(),
            Self::Tabs => sidebar_icon(color, size).into_any_element(),
            Self::Sites => globe_icon(color, size).into_any_element(),
            Self::Privacy => shield_icon(color, size).into_any_element(),
        }
    }
}

/// The settings page.
pub(super) const PAGE: PageDefinition = PageDefinition {
    name: "settings",
    title: "Settings",
    icon: PageIcon::Symbol(|color, size| settings_icon(color, size).into_any_element()),
    shows_address: true,
    single_tab: true,
    build: |context, cx| cx.new(|cx| SettingsPage::new(context, cx)).into(),
};

pub(in super::super) struct SettingsPage {
    context: PageContext,
    section: Section,
    /// When a section was last picked from the sidebar.
    section_picked_at: Option<Instant>,
    clearing: ClearForm,
    /// Whether the dialog asking to reset every setting is open.
    confirming_reset: bool,
    reset_presence: Presence<()>,
    reset_focus: FocusHandle,
    _settings_observer: Subscription,
}

impl SettingsPage {
    fn new(context: PageContext, cx: &mut Context<Self>) -> Self {
        Self {
            context,
            section: Section::Appearance,
            section_picked_at: None,
            clearing: ClearForm::new(cx),
            confirming_reset: false,
            reset_presence: Presence::default(),
            reset_focus: cx.focus_handle(),
            // Follow changes made here, on the new tab page, or in another window.
            _settings_observer: cx.observe_global::<Settings>(|_, cx| cx.notify()),
        }
    }

    fn show(&mut self, section: Section, cx: &mut Context<Self>) {
        if self.section != section {
            self.section = section;
            self.section_picked_at = Some(Instant::now());
            cx.notify();
        }
    }
}

impl Render for SettingsPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = palette(window, cx);
        let section = self.section;
        let body = match section {
            Section::Appearance => appearance::settings(palette, cx).into_any_element(),
            Section::Search => search::settings(palette, cx).into_any_element(),
            Section::NewTab => new_tab::settings(palette, cx).into_any_element(),
            Section::Tabs => tabs::settings(palette, cx).into_any_element(),
            Section::Sites => sites::settings(palette, cx).into_any_element(),
            Section::Privacy => self.privacy_settings(palette, cx).into_any_element(),
        };
        let content = v_stack()
            .w_full()
            .gap(px(metrics::INTERNAL_PAGE_GROUP_GAP))
            .child(heading(
                section.id(),
                section.title(),
                section.description(),
                palette,
            ))
            .child(body);
        // A section you pick fades in as it replaces the last; one shown
        // again, as when switching back to this tab, appears at once.
        let content = match self.section_picked_at {
            Some(picked) if picked.elapsed() < SECTION_MOTION.total_duration() => content
                .animate_in(
                    (ElementId::from("settings-section"), section.id()),
                    SECTION_MOTION,
                )
                .into_any_element(),
            _ => content.into_any_element(),
        };
        let reset_dialog = self.reset_dialog(palette, cx);
        h_stack()
            .relative()
            .size_full()
            .items_start()
            .child(self.sidebar(palette, cx))
            .child(
                div()
                    .flex_1()
                    .h_full()
                    .min_w_0()
                    .child(scrolling_column("settings-content", content)),
            )
            .children(reset_dialog)
    }
}

impl SettingsPage {
    fn sidebar(&self, palette: ThemeColors, cx: &mut Context<Self>) -> impl IntoElement {
        let items = Section::ALL.into_iter().map(|section| {
            let selected = section == self.section;
            let color = if selected {
                palette.text_primary
            } else {
                palette.text_secondary
            };
            let item = h_stack()
                .id((ElementId::from("settings-sidebar"), section.id()))
                .role(Role::Tab)
                .aria_label(section.title())
                .aria_selected(selected)
                .tab_index(0)
                .focus_visible(|style| style.border_1().border_color(rgb(palette.chosen)))
                .items_center()
                .gap(px(metrics::OMNIBOX_GAP))
                .h(px(metrics::SETTINGS_SIDEBAR_ITEM_HEIGHT))
                .px(px(metrics::MENU_ITEM_HORIZONTAL_PADDING))
                .rounded(px(metrics::CONTROL_RADIUS))
                .text_size(px(metrics::BUTTON_FONT_SIZE))
                .text_color(rgb(color))
                .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                .on_click(cx.listener(move |this, _, _, cx| this.show(section, cx)))
                .child(section.icon(color))
                .child(section.title());
            if selected {
                item.bg(rgba(palette.selected_surface))
                    .font_weight(FontWeight::MEDIUM)
            } else {
                item.hover(|style| style.bg(rgba(palette.hover_surface)))
            }
        });
        let saved_note = if Settings::is_saved(cx) {
            "Saved to this profile"
        } else {
            "Not saved: temporary session"
        };
        v_stack()
            .id("settings-sidebar")
            .role(Role::TabList)
            .aria_label("Settings sections")
            .flex_shrink_0()
            .h_full()
            .w(px(metrics::SETTINGS_SIDEBAR_WIDTH))
            .gap(px(metrics::MENU_ITEM_GAP))
            .p(px(metrics::INTERNAL_PAGE_GROUP_GAP))
            .border_r_1()
            .border_color(rgba(palette.menu_border))
            .child(
                div()
                    .px(px(metrics::MENU_ITEM_HORIZONTAL_PADDING))
                    .pb(px(metrics::INTERNAL_PAGE_GROUP_GAP))
                    .text_size(px(metrics::INTERNAL_PAGE_SECTION_SIZE))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(palette.text_primary))
                    .child("Settings"),
            )
            .children(items)
            .child(div().flex_1())
            .child(
                v_stack()
                    .items_start()
                    .gap(px(metrics::MENU_ITEM_GAP))
                    .px(px(metrics::MENU_ITEM_HORIZONTAL_PADDING))
                    .child(self.reset_button(palette, cx))
                    .child(secondary_text(saved_note, palette)),
            )
    }
}
