//! Spaces in the window: which space's tabs the sidebar shows, switching by
//! the footer's dots, a two-finger swipe or the keyboard, and creating,
//! changing and removing spaces.

use gpui::{
    AnyElement, Context, Entity, MouseDownEvent, ScrollDelta, ScrollWheelEvent, Subscription,
    TouchPhase, Window, div, prelude::*,
};
use photon_core::{SpaceId, TabLayout, WindowColor};
use std::time::Instant;

use super::super::layout::v_stack;
use super::super::menu::{menu_action, menu_surface};
use super::super::modal::{MODAL_MOTION, modal};
use super::super::motion::{Edge, Presence, Transition};
use super::super::pages::NEW_TAB;
use super::super::settings::Settings;
use super::super::sidebar::SpaceDot;
use super::super::space_editor::{SpaceEditor, SpaceEditorEvent};
use super::super::theme::{ThemeColors, window_color_swatch};
use super::BrowserWindow;
use super::menu::OpenMenu;

/// How far two fingers move sideways across the sidebar to switch space.
const SWIPE_DISTANCE: f32 = 80.0;

/// The window's view of the spaces.
pub(super) struct SpacesState {
    /// The space the window last showed, to notice when it changes.
    shown: SpaceId,
    /// The edge the shown space came in from, and when, so it slides in
    /// only just after switching.
    switched: Option<(Edge, Instant)>,
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
            shown,
            switched: None,
            swipe: 0.0,
            swiped: false,
            editor: None,
            editor_presence: Presence::default(),
        }
    }

    /// The edge to slide the sidebar in from, just after switching space.
    pub(super) fn arriving_from(&self, duration: std::time::Duration) -> Option<Edge> {
        self.switched
            .filter(|(_, at)| at.elapsed() < duration)
            .map(|(edge, _)| edge)
    }
}

impl BrowserWindow {
    /// The space new tabs join, and whose tabs the sidebar shows.
    pub(super) fn current_space(&self, cx: &gpui::App) -> SpaceId {
        Settings::get(cx).spaces.active().id
    }

    /// Whether the window shows one space at a time: in the sidebar. The tab
    /// strip shows every tab.
    pub(super) fn shows_spaces(&self, cx: &gpui::App) -> bool {
        Settings::get(cx).tab_layout == TabLayout::Vertical
    }

    pub(super) fn switch_space(&mut self, id: SpaceId, cx: &mut Context<Self>) {
        Settings::update(cx, |settings| {
            settings.spaces.activate(id);
        });
    }

    /// Switches to the space `offset` places along, if there is one.
    pub(super) fn switch_space_by(&mut self, offset: isize, cx: &mut Context<Self>) {
        if let Some(id) = Settings::get(cx).spaces.neighbour(offset) {
            self.switch_space(id, cx);
        }
    }

    /// Follows the space shown, which any window may switch: slides the
    /// sidebar in from the side it came from, and shows that space's most
    /// recent tab, or a new tab page in a space with none.
    pub(super) fn follow_space(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let spaces = &Settings::get(cx).spaces;
        let active = spaces.active().id;
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
        self.spaces.switched = Some((edge, Instant::now()));
        if !self.shows_spaces(cx) || self.tabs[self.active_tab].space == active {
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
            None => self.insert_page(self.tabs.len(), NEW_TAB, window, cx),
        }
    }

    /// Switches space with a two-finger swipe across the sidebar, once per
    /// swipe.
    pub(super) fn swipe_spaces(&mut self, event: &ScrollWheelEvent, cx: &mut Context<Self>) {
        let ScrollDelta::Pixels(delta) = event.delta else {
            return;
        };
        let (x, y) = (f32::from(delta.x), f32::from(delta.y));
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
        let spaces = &Settings::get(cx).spaces;
        let active = spaces.active().id;
        spaces
            .all()
            .iter()
            .map(|space| {
                let id = space.id;
                SpaceDot {
                    name: space.name.clone(),
                    color: match space.color {
                        WindowColor::System => palette.text_secondary,
                        color => window_color_swatch(color, palette),
                    },
                    active: id == active,
                    on_select: Box::new(cx.listener(move |this, _, _, cx| {
                        this.switch_space(id, cx);
                    })),
                    on_context_menu: Box::new(cx.listener(
                        move |this, event: &MouseDownEvent, _, cx| {
                            cx.stop_propagation();
                            this.open_menu = Some(OpenMenu::Space(id, event.position));
                            cx.notify();
                        },
                    )),
                }
            })
            .collect()
    }

    /// The menu for the space `id`: change it, or remove it unless it is the
    /// last.
    pub(super) fn space_menu(
        &self,
        id: SpaceId,
        transition: Transition,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let removable = Settings::get(cx).spaces.all().len() > 1;
        let content = v_stack()
            .child(menu_action(
                "space-menu-edit",
                "Edit Space…",
                0,
                Box::new(cx.listener(move |this, _, window, cx| {
                    cx.stop_propagation();
                    this.open_menu = None;
                    this.edit_space(Some(id), window, cx);
                })),
                palette,
            ))
            .when(removable, |menu| {
                menu.child(menu_action(
                    "space-menu-delete",
                    "Delete Space",
                    1,
                    Box::new(cx.listener(move |this, _, window, cx| {
                        cx.stop_propagation();
                        this.open_menu = None;
                        this.delete_space(id, window, cx);
                    })),
                    palette,
                ))
            });
        menu_surface(content, transition, palette)
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
            gradient,
        } = event
        {
            let (name, color, gradient) = (name.clone(), *color, *gradient);
            Settings::update(cx, |settings| match editing.space {
                Some(id) => {
                    if let Some(space) = settings.spaces.get_mut(id) {
                        if !name.is_empty() {
                            space.name = name;
                        }
                        space.color = color;
                        space.gradient = gradient;
                    }
                }
                None => {
                    let id = settings.spaces.add(Some(name), color, gradient);
                    settings.spaces.activate(id);
                }
            });
        }
        self.activate_tab(self.active_tab, true, window, cx);
        cx.notify();
    }

    /// Removes the space `id` and closes its tabs, after switching away from
    /// it when it is shown.
    fn delete_space(&mut self, id: SpaceId, window: &mut Window, cx: &mut Context<Self>) {
        let mut removed = false;
        Settings::update(cx, |settings| removed = settings.spaces.remove(id));
        if !removed {
            return;
        }
        self.follow_space(window, cx);
        for index in (0..self.tabs.len()).rev() {
            if self.tabs[index].space == id && self.tabs.len() > 1 {
                self.close_tab(index, window, cx);
            }
        }
        self.save_pinned_tabs(cx);
    }

    /// The space dialog while open or closing, over the whole window.
    pub(super) fn space_editor_overlay(
        &mut self,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let open = self
            .spaces
            .editor
            .as_ref()
            .map(|editing| editing.editor.clone());
        let (editor, transition) = self.spaces.editor_presence.sync(open, MODAL_MOTION, cx)?;
        Some(
            modal(
                "space-editor-modal",
                transition,
                palette,
                div().child(editor),
            )
            .into_any_element(),
        )
    }
}
