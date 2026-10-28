//! Spaces in the window: which space's tabs the sidebar shows, switching by
//! the footer's dots, a two-finger swipe or the keyboard, and creating,
//! changing and removing spaces.

use gpui::{
    AnyElement, Context, Entity, MenuItem, MouseDownEvent, ScrollDelta, ScrollWheelEvent,
    Subscription, TouchPhase, Window, div, prelude::*,
};
use photon_core::{SpaceId, WindowColor};
use std::time::Instant;
use std::{cell::Cell, rc::Rc};

use super::super::input::physical_horizontal_delta;
use super::super::modal::{MODAL_MOTION, modal};
use super::super::motion::{Edge, Presence};
use super::super::pages::NEW_TAB;
use super::super::settings::Settings;
use super::super::sidebar::SpaceDot;
use super::super::space_editor::{SpaceEditor, SpaceEditorEvent};
use super::super::theme::{ThemeColors, window_color_swatch};
use super::super::window_space::{resolve_space, set_window_space};
use super::super::workspace_layout::WorkspaceLayout;
use super::BrowserWindow;
use super::menu::{MenuCommand, command_item};
use super::registry::BrowserWindowRegistry;

/// How far two fingers move sideways across the sidebar to switch space.
const SWIPE_DISTANCE: f32 = 80.0;

/// The window's view of the spaces.
pub(super) struct SpacesState {
    /// The space this window shows. Each window has its own; the settings'
    /// active space is only where new windows open.
    current: SpaceId,
    /// The space the window last showed, to notice when it changes.
    shown: SpaceId,
    /// The edge the shown space came in from, and when, so it slides in
    /// only just after switching while the sidebar is already settled open.
    switched: Cell<Option<(Edge, Instant)>>,
    /// How far the current two-finger swipe has moved, and whether it has
    /// already switched.
    swipe: f32,
    swiped: bool,
    /// The dialog for a new space or one being changed.
    editor: Option<SpaceEditing>,
    editor_presence: Presence<Entity<SpaceEditor>>,
}

struct SpaceEditing {
    editor: Entity<SpaceEditor>,
    /// The space being changed; `None` while creating one.
    space: Option<SpaceId>,
    _subscription: Subscription,
}

impl SpacesState {
    pub(super) fn new(shown: SpaceId) -> Self {
        Self {
            current: shown,
            shown,
            switched: Cell::new(None),
            swipe: 0.0,
            swiped: false,
            editor: None,
            editor_presence: Presence::default(),
        }
    }

    /// The edge to slide the sidebar in from, just after switching space.
    pub(super) fn arriving_from(
        &self,
        duration: std::time::Duration,
        allowed: bool,
    ) -> Option<Edge> {
        self.switched
            .take()
            .filter(|(_, at)| allowed && at.elapsed() < duration)
            .map(|(edge, _)| edge)
    }
}

impl BrowserWindow {
    /// The space new tabs join, and whose tabs the sidebar shows.
    pub(super) fn current_space(&self, cx: &gpui::App) -> SpaceId {
        resolve_space(self.spaces.current, cx).id
    }

    /// The workspace layout in the window's space.
    pub(super) fn layout(&self, cx: &gpui::App) -> std::rc::Rc<WorkspaceLayout> {
        WorkspaceLayout::current(self.current_space(cx), cx)
    }

    /// Shows `id`, while it exists, without sliding to it, as when a window
    /// reopens in the space it showed.
    #[cfg_attr(feature = "ui-only", allow(dead_code))]
    pub(super) fn show_space_immediately(&mut self, id: SpaceId, cx: &mut Context<Self>) {
        if Settings::get(cx).spaces.get(id).is_some() {
            self.spaces.shown = id;
            self.set_current_space(id, cx);
        }
    }

    /// Records `id` as the space this window shows.
    fn set_current_space(&mut self, id: SpaceId, cx: &mut Context<Self>) {
        self.spaces.current = id;
        set_window_space(self.window_id, id, cx);
    }

    /// Whether the window shows one space at a time: in the sidebar. The tab
    /// strip shows every tab.
    pub(super) fn shows_spaces(&self, cx: &gpui::App) -> bool {
        self.layout(cx).shows_spaces()
    }

    pub(super) fn switch_space(&mut self, id: SpaceId, cx: &mut Context<Self>) {
        if Settings::get(cx).spaces.get(id).is_none() {
            return;
        }
        self.set_current_space(id, cx);
        // New windows open in the space last chosen. Updating the settings
        // also redraws every view in this window, which follows its space.
        Settings::update(cx, |settings| {
            settings.spaces.activate(id);
        });
    }

    /// Switches to the space `offset` places along, if there is one.
    pub(super) fn switch_space_by(&mut self, offset: isize, cx: &mut Context<Self>) {
        let current = self.current_space(cx);
        if let Some(id) = Settings::get(cx).spaces.neighbour_of(current, offset) {
            self.switch_space(id, cx);
        }
    }

    /// Follows the space this window shows: falls back when it was deleted,
    /// slides the sidebar in from the side it came from, and shows that
    /// space's most recent tab, or a new tab page in a space with none.
    pub(super) fn follow_space(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let active = self.current_space(cx);
        if active != self.spaces.current {
            self.set_current_space(active, cx);
        }
        let spaces = &Settings::get(cx).spaces;
        if self.theme_picker_space.is_some_and(|space| space != active) {
            self.theme_picker_space = None;
        }
        if self.spaces.shown == active {
            return;
        }
        let position = |id| spaces.all().iter().position(|space| space.id == id);
        let edge = if position(active) > position(self.spaces.shown) {
            Edge::Right
        } else {
            Edge::Left
        };
        self.spaces.shown = active;
        let sidebar_is_settled = self.sidebar.docked()
            && !self.sidebar.over_page()
            && !self.sidebar.is_moving()
            && self.sidebar.shown() >= 1.0;
        self.spaces
            .switched
            .set(sidebar_is_settled.then_some((edge, Instant::now())));
        let shows_spaces = self.shows_spaces(cx);
        if shows_spaces {
            // The space container has its own entrance. Drop pending row
            // entrances so remounting tabs cannot replay their springs.
            self.tab_entry_animations.borrow_mut().clear();
        }
        let Some(active_tab) = self.active_tab else {
            cx.notify();
            return;
        };
        if !shows_spaces || self.tabs[active_tab].space == active {
            cx.notify();
            return;
        }
        let latest = self
            .tabs
            .iter()
            .enumerate()
            .filter(|(_, tab)| tab.space == active)
            .max_by_key(|(_, tab)| tab.last_active)
            .map(|(index, _)| index);
        match latest {
            Some(index) => self.activate_tab(index, true, window, cx),
            None if shows_spaces => {
                self.insert_page_immediately(self.new_tab_index(cx), NEW_TAB, window, cx);
            }
            None => self.insert_page(self.new_tab_index(cx), NEW_TAB, window, cx),
        }
    }

    /// Switches space with a two-finger swipe across the sidebar, once per
    /// swipe.
    pub(super) fn swipe_spaces(&mut self, event: &ScrollWheelEvent, cx: &mut Context<Self>) {
        let ScrollDelta::Pixels(delta) = event.delta else {
            return;
        };
        let (x, y) = (physical_horizontal_delta(event), f32::from(delta.y));
        if event.touch_phase == TouchPhase::Started {
            self.spaces.swipe = 0.0;
            self.spaces.swiped = false;
        }
        if self.spaces.swiped || x.abs() <= y.abs() {
            return;
        }
        self.spaces.swipe += x;
        if self.spaces.swipe.abs() >= SWIPE_DISTANCE {
            self.spaces.swiped = true;
            // Fingers moving left bring in the space to the right.
            self.switch_space_by(if self.spaces.swipe < 0.0 { 1 } else { -1 }, cx);
        }
    }

    /// The footer's dots, one per space.
    pub(super) fn space_dots(&self, palette: ThemeColors, cx: &mut Context<Self>) -> Vec<SpaceDot> {
        let active = self.current_space(cx);
        let spaces = &Settings::get(cx).spaces;
        spaces
            .all()
            .iter()
            .map(|space| {
                let id = space.id;
                let on_select: super::super::ClickHandler =
                    Box::new(cx.listener(move |this, _, _, cx| {
                        this.switch_space(id, cx);
                    }));
                SpaceDot {
                    id,
                    name: space.name.clone(),
                    color: space.tint.map_or_else(
                        || match space.color {
                            WindowColor::System => palette.text_secondary,
                            color => window_color_swatch(color),
                        },
                        super::super::theme::tint_swatch,
                    ),
                    active: id == active,
                    on_select: Rc::new(on_select),
                    on_context_menu: Box::new(cx.listener(
                        move |this, _: &MouseDownEvent, window, cx| {
                            cx.stop_propagation();
                            let items = this.space_menu_items(id, cx);
                            this.show_context_menu(window, items, cx);
                        },
                    )),
                }
            })
            .collect()
    }

    /// The menu for the space `id`: edit it, or delete it when it is not
    /// the only one.
    fn space_menu_items(&self, id: SpaceId, cx: &Context<Self>) -> Vec<MenuItem> {
        let mut items = vec![command_item("Edit Space…", MenuCommand::EditSpace(id))];
        if Settings::get(cx).spaces.all().len() > 1 {
            items.push(command_item("Delete Space", MenuCommand::DeleteSpace(id)));
        }
        items
    }

    /// Opens the dialog for the space `id`, or for a new space.
    pub(super) fn edit_space(
        &mut self,
        id: Option<SpaceId>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let space = id.and_then(|id| Settings::get(cx).spaces.get(id).cloned());
        let editor = cx.new(|cx| SpaceEditor::new(space.as_ref(), window, cx));
        let subscription = cx.subscribe_in(
            &editor,
            window,
            |this, _, event: &SpaceEditorEvent, window, cx| this.space_edited(event, window, cx),
        );
        self.spaces.editor = Some(SpaceEditing {
            editor,
            space: id,
            _subscription: subscription,
        });
        cx.notify();
    }

    fn space_edited(
        &mut self,
        event: &SpaceEditorEvent,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let Some(editing) = self.spaces.editor.take() else {
            return;
        };
        if let SpaceEditorEvent::Save {
            name,
            color,
            colors,
            transparency,
            noise,
            preset_id,
            new_preset,
        } = event
        {
            let (name, color, colors, transparency, noise, preset_id, new_preset) = (
                name.clone(),
                *color,
                *colors,
                *transparency,
                *noise,
                preset_id.clone(),
                new_preset.clone(),
            );
            let creating = editing.space.is_none();
            let previous_name = editing
                .space
                .and_then(|id| Settings::get(cx).spaces.get(id))
                .map(|space| space.name.clone());
            let created_preset = new_preset.is_some();
            let mut saved = false;
            let mut created = None;
            Settings::update(cx, |settings| {
                let preset_id = new_preset
                    .clone()
                    .and_then(|(name, layout)| settings.workspace.add_preset(&name, layout))
                    .or(preset_id.clone().filter(|id| {
                        settings
                            .workspace
                            .presets
                            .iter()
                            .any(|preset| preset.id == *id)
                    }));
                let space_id = match editing.space {
                    Some(id) => {
                        let Some(space) = settings.spaces.get_mut(id) else {
                            return;
                        };
                        if !name.is_empty() {
                            space.name = name.clone();
                        }
                        space.color = color;
                        space.set_colors(colors);
                        id
                    }
                    None => {
                        let id = settings.spaces.add(
                            Some(name.clone()),
                            color,
                            colors.colors().len() > 1,
                        );
                        // The new space opens in this window and the next
                        // new windows, not in other open windows.
                        settings.spaces.activate(id);
                        created = Some(id);
                        id
                    }
                };
                if let Some(space) = settings.spaces.get_mut(space_id) {
                    space.set_colors(colors);
                }
                if let Some(transparency) = transparency {
                    let active_has_colors = !super::super::theme::space_colors(
                        settings
                            .spaces
                            .get(space_id)
                            .unwrap_or_else(|| settings.spaces.active()),
                    )
                    .colors()
                    .is_empty();
                    if !active_has_colors
                        || transparency
                            == super::super::appearance_preview::AppearancePreview::subtle_transparency()
                    {
                        settings.transparency = photon_core::Transparency::Subtle;
                        settings.transparency_percent = None;
                    } else {
                        settings.transparency_percent = Some(transparency);
                    }
                }
                if let Some(noise) = noise {
                    settings.background_noise_intensity = noise;
                }
                if let Some(preset_id) = preset_id {
                    settings.spaces.set_preset_override(space_id, &preset_id);
                } else {
                    settings.spaces.clear_layout_override(space_id);
                }
                saved = true;
            });
            if let Some(id) = created {
                self.set_current_space(id, cx);
            }
            if saved {
                let renamed = previous_name
                    .as_deref()
                    .is_some_and(|previous| !name.trim().is_empty() && previous != name.trim());
                let message = if created_preset {
                    if creating {
                        "Space created with a layout preset".to_owned()
                    } else {
                        "Space and layout preset saved".to_owned()
                    }
                } else if creating {
                    "Space created".to_owned()
                } else if renamed {
                    format!("Space renamed to “{}”", name.trim())
                } else {
                    "Space updated".to_owned()
                };
                self.show_action_alert(message, cx);
            }
        }
        if let Some(active_tab) = self.active_tab {
            self.activate_tab(active_tab, true, window, cx);
        }
        cx.notify();
    }

    /// Removes the space `id` and closes its tabs, after switching away from
    /// it when it is shown.
    pub(super) fn delete_space(
        &mut self,
        id: SpaceId,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = Settings::get(cx)
            .spaces
            .get(id)
            .map(|space| space.name.clone());
        // Windows showing the space move to the one after it, or before it
        // when it was last.
        let fallback = {
            let spaces = Settings::get(cx).spaces.all();
            let index = spaces.iter().position(|space| space.id == id);
            index.and_then(|index| {
                spaces
                    .get(index + 1)
                    .or_else(|| index.checked_sub(1).and_then(|index| spaces.get(index)))
                    .map(|space| space.id)
            })
        };
        let mut removed = false;
        Settings::update(cx, |settings| removed = settings.spaces.remove(id));
        if !removed {
            return;
        }
        self.show_action_alert(
            name.map_or_else(
                || "Space deleted".to_owned(),
                |name| format!("Deleted space “{name}”"),
            ),
            cx,
        );
        self.close_deleted_space_tabs(id, fallback, window, cx);
        // Other windows lose their tabs in the space the same way.
        let current = window.window_handle().window_id();
        for handle in BrowserWindowRegistry::ordered_handles(cx) {
            if handle.window_id() == current {
                continue;
            }
            if let Some(other) = handle.downcast::<BrowserWindow>() {
                let _ = other.update(cx, |browser, window, cx| {
                    browser.close_deleted_space_tabs(id, fallback, window, cx);
                });
            }
        }
    }

    /// Moves to `fallback` if the window showed `id`, which was deleted, and
    /// closes the tabs left in it, keeping the window's last tab.
    fn close_deleted_space_tabs(
        &mut self,
        id: SpaceId,
        fallback: Option<SpaceId>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if self.spaces.current == id
            && let Some(fallback) = fallback
        {
            self.set_current_space(fallback, cx);
        }
        self.follow_space(window, cx);
        for index in (0..self.tabs.len()).rev() {
            if self.tabs[index].space == id && self.tabs.len() > 1 {
                self.close_tab(index, window, cx);
            }
        }
    }

    /// The space dialog while open or closing, over the whole window.
    pub(super) fn space_editor_overlay(
        &mut self,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let animations = Settings::motion(cx);
        let open = self
            .spaces
            .editor
            .as_ref()
            .map(|editing| editing.editor.clone());
        let (editor, transition) =
            self.spaces
                .editor_presence
                .sync(open, MODAL_MOTION, &animations, cx)?;
        Some(
            modal(
                "space-editor-modal",
                transition,
                palette,
                div().child(editor),
                cx,
            )
            .into_any_element(),
        )
    }
}
