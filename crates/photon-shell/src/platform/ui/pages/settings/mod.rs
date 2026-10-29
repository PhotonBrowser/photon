//! The settings page: a sidebar of sections and the chosen section's
//! settings.
//!
//! Each section lives in its own module and draws from the shared
//! [`Settings`]; only clearing browsing data keeps state of its own.
//! `reset` returns every setting to its default, after asking.

mod appearance;
mod general;
mod new_tab;
mod privacy;
mod reset;
mod search;
mod sites;
mod tabs;
mod workspace_presets;

use super::super::controls::{FocusRing, RovingAxis, RovingFocus};
use super::super::controls::{OwnsClicks, Pressable};
use super::super::icons::{
    appearance_icon, globe_icon, grid_icon, search_icon_sized, settings_icon, shield_icon,
    sidebar_icon,
};
use super::super::layout::{h_stack, v_stack};
use super::super::motion::{AnimateIn, Entrance, Presence, fade};
use super::super::settings::Settings;
use super::super::{ClickHandler, metrics, theme::ThemeColors, theme::page_palette as palette};
use super::layout::{heading, scrolling_column, secondary_text};
use super::{PageContext, PageDefinition, PageIcon};
use gpui::{
    AnyElement, App, Context, ElementId, FocusHandle, FontWeight, Render, Role, Subscription,
    Window, div, prelude::*, px, rgb, rgba,
};
use gpui_elements::editable_text::EditableTextState;
use photon_core::{SidebarSide, TabLayout, WorkspacePreset};
use privacy::ClearForm;
use std::rc::Rc;
use std::time::Instant;

/// How a picked section appears.
const SECTION_MOTION: Entrance = fade();

/// A part of the settings, shown on its own.
#[derive(Clone, Copy, PartialEq)]
enum Section {
    General,
    Appearance,
    Search,
    NewTab,
    Tabs,
    Sites,
    Privacy,
}

impl Section {
    const ALL: [Self; 7] = [
        Self::General,
        Self::Appearance,
        Self::Search,
        Self::NewTab,
        Self::Tabs,
        Self::Sites,
        Self::Privacy,
    ];

    fn id(self) -> &'static str {
        match self {
            Self::General => "general",
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
            Self::General => "General",
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
            Self::General => format!("How {name} fits in with your Mac."),
            Self::Appearance => format!("How {name} looks."),
            Self::Search => {
                "Where searches from the address bar go, and how it shows addresses.".to_owned()
            }
            Self::NewTab => "What new tabs show.".to_owned(),
            Self::Tabs => "How tabs open and close.".to_owned(),
            Self::Sites => "What sites may do.".to_owned(),
            Self::Privacy => format!("Delete what {name} and websites keep on this Mac."),
        }
    }

    fn icon(self, color: u32) -> AnyElement {
        let size = metrics::SETTINGS_SIDEBAR_ICON_SIZE;
        match self {
            Self::General => settings_icon(color, size).into_any_element(),
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
    workspace_preset_draft: Option<WorkspacePresetDraft>,
    workspace_preset_presence: Presence<()>,
    pending_delete_preset: Option<WorkspacePreset>,
    delete_preset_presence: Presence<()>,
    delete_preset_focus: FocusHandle,
    /// What had focus before a dialog took it, given back when it closes.
    dialog_return_focus: Option<FocusHandle>,
    _settings_observer: Subscription,
}

struct WorkspacePresetDraft {
    preset_id: Option<String>,
    name: gpui::Entity<EditableTextState>,
    tab_layout: TabLayout,
    sidebar_side: SidebarSide,
    invalid: bool,
    _name_observer: Subscription,
}

impl SettingsPage {
    /// Remembers what has focus, before a dialog takes it.
    fn remember_dialog_focus(&mut self, window: &Window, cx: &App) {
        self.dialog_return_focus = window.focused(cx);
    }

    /// Gives focus back to what had it before a dialog opened.
    fn restore_dialog_focus(&mut self, window: &mut Window, cx: &mut App) {
        if let Some(focus) = self.dialog_return_focus.take() {
            window.focus(&focus, cx);
        }
    }

    fn new(context: PageContext, cx: &mut Context<Self>) -> Self {
        general::refresh_default_browser(cx);
        Self {
            context,
            section: Section::General,
            section_picked_at: None,
            clearing: ClearForm::new(cx),
            confirming_reset: false,
            reset_presence: Presence::default(),
            reset_focus: cx.focus_handle(),
            workspace_preset_draft: None,
            workspace_preset_presence: Presence::default(),
            pending_delete_preset: None,
            delete_preset_presence: Presence::default(),
            delete_preset_focus: cx.focus_handle(),
            dialog_return_focus: None,
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
        let create_preset =
            Box::new(cx.listener(|this, _, window, cx| this.start_workspace_preset(window, cx)));
        let page = cx.entity().downgrade();
        let edit_preset: Rc<dyn Fn(String) -> ClickHandler> = Rc::new(move |id| {
            let page = page.clone();
            Box::new(move |_, window, cx| {
                page.update(cx, |this, cx| this.edit_workspace_preset(&id, window, cx))
                    .ok();
            })
        });
        let page = cx.entity().downgrade();
        let delete_preset: Rc<dyn Fn(String) -> ClickHandler> = Rc::new(move |id| {
            let page = page.clone();
            Box::new(move |_, window, cx| {
                page.update(cx, |this, cx| {
                    this.ask_to_delete_workspace_preset(&id, window, cx)
                })
                .ok();
            })
        });
        let body = match section {
            Section::General => general::settings(palette, cx).into_any_element(),
            Section::Appearance => appearance::settings(
                window,
                palette,
                cx,
                create_preset,
                edit_preset,
                delete_preset,
            )
            .into_any_element(),
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
            Some(picked)
                if picked.elapsed() < SECTION_MOTION.total_duration(&Settings::motion(cx)) =>
            {
                content
                    .animate_in(
                        (ElementId::from("settings-section"), section.id()),
                        SECTION_MOTION,
                        &Settings::motion(cx),
                    )
                    .into_any_element()
            }
            _ => content.into_any_element(),
        };
        let menu_palette = super::super::theme::palette(window, cx);
        let reset_dialog = self.reset_dialog(menu_palette, cx);
        let workspace_preset_dialog = self.workspace_preset_dialog(menu_palette, cx);
        let workspace_preset_delete_dialog = self.workspace_preset_delete_dialog(menu_palette, cx);
        h_stack()
            .text_color(rgb(palette.text_primary))
            .relative()
            .size_full()
            .items_start()
            .child(self.sidebar(window, palette, cx))
            .child(div().flex_1().h_full().min_w_0().child(scrolling_column(
                "settings-content",
                content,
                crate::platform::supports_elastic_scroll() && Settings::get(cx).elastic_scroll,
                if section == Section::Appearance {
                    metrics::SETTINGS_APPEARANCE_MAX_WIDTH
                } else {
                    metrics::INTERNAL_PAGE_MAX_WIDTH
                },
            )))
            .children(reset_dialog)
            .children(workspace_preset_dialog)
            .children(workspace_preset_delete_dialog)
    }
}

impl SettingsPage {
    fn sidebar(
        &self,
        window: &mut Window,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let selected_index = Section::ALL
            .iter()
            .position(|section| *section == self.section)
            .unwrap_or(0);
        let actions: Vec<Rc<ClickHandler>> = Section::ALL
            .into_iter()
            .map(|section| {
                let action: ClickHandler =
                    Box::new(cx.listener(move |this, _, _, cx| this.show(section, cx)));
                Rc::new(action)
            })
            .collect();
        let focus = RovingFocus::new("settings-sections", selected_index, actions, window, cx);
        let items = Section::ALL
            .into_iter()
            .enumerate()
            .map(|(index, section)| {
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
                    .track_focus(&focus.handle(index))
                    .tab_index(index as isize)
                    .tab_stop(focus.selected(index))
                    .focus_ring(palette)
                    .items_center()
                    .gap(px(metrics::OMNIBOX_GAP))
                    .h(px(metrics::SETTINGS_SIDEBAR_ITEM_HEIGHT))
                    .px(px(metrics::MENU_ITEM_HORIZONTAL_PADDING))
                    .rounded(px(metrics::control_radius()))
                    .text_size(px(metrics::BUTTON_FONT_SIZE))
                    .text_color(rgb(color))
                    .owns_clicks()
                    .pressable()
                    .on_click(focus.on_click(index))
                    .on_key_down(focus.on_key_down(index, RovingAxis::Vertical))
                    .child(section.icon(color))
                    .child(section.title());
                if selected {
                    item.bg(rgba(palette.selected_surface))
                        .font_weight(FontWeight::MEDIUM)
                } else {
                    item.hover_style(|style| style.bg(rgba(palette.hover_surface)))
                }
            });
        let saved_note = if Settings::is_saved(cx) {
            "Saved to this profile"
        } else {
            "Not saved: temporary session"
        };
        let section_tabs = v_stack()
            .id("settings-sections")
            .role(Role::TabList)
            .aria_label("Settings sections")
            .aria_orientation(gpui::Orientation::Vertical)
            .tab_group()
            .gap(px(metrics::MENU_ITEM_GAP))
            .children(items);
        v_stack()
            .id("settings-sidebar")
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
            .child(section_tabs)
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
