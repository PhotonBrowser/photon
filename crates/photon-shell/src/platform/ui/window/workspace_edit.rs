//! Draft editor for the current space's browser workspace.

use gpui::{
    AnyElement, Context, ElementId, InteractiveElement, MouseButton, Render, Role, SharedString,
    Window, div, prelude::*, px, rgb, rgba,
};
use photon_core::{DockRegion, LayoutComponent, WorkspaceLayout as SavedWorkspaceLayout};

use super::super::layout::{Elevated, Elevation, Raised, h_stack, v_stack};
use super::super::motion::{AnimateIn, Edge, Entrance, Speed, distance};
use super::super::settings::Settings;
use super::super::sidebar::navigation_bar;
use super::super::theme::ThemeColors;
use super::super::workspace_layout::{
    ComponentDefinition, ComponentId, WorkspaceLayout, component_catalog, component_definition,
    component_group_bounds, component_group_keeps_side_members_together, component_groups,
};
use super::super::{ClickHandler, metrics};
use super::BrowserWindow;

#[derive(Clone)]
pub(super) struct WorkspaceEditing {
    space_id: photon_core::SpaceId,
    space_name: String,
    profile_default: SavedWorkspaceLayout,
    draft: SavedWorkspaceLayout,
    undo: Option<SavedWorkspaceLayout>,
}

struct WorkspaceComponentDrag {
    id: String,
}

struct WorkspaceDragPreview {
    label: SharedString,
    palette: ThemeColors,
}

impl Render for WorkspaceDragPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        h_stack()
            .items_center()
            .h(px(metrics::BUTTON_HEIGHT))
            .px(px(metrics::BUTTON_HORIZONTAL_PADDING))
            .rounded(px(metrics::CONTROL_RADIUS))
            .border_1()
            .border_color(rgba(self.palette.menu_border))
            .raised(self.palette)
            .text_color(rgb(self.palette.text_primary))
            .elevated(Elevation::Medium)
            .child(self.label.clone())
    }
}

impl BrowserWindow {
    pub(super) fn open_workspace_editor(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let settings = Settings::get(cx);
        let space = settings.spaces.active();
        let profile_default = settings
            .workspace
            .default_preset()
            .and_then(|preset| WorkspaceLayout::from_saved(&preset.layout))
            .map(|layout| layout.saved().clone())
            .unwrap_or_else(|| {
                WorkspaceLayout::for_tab_layout(settings.tab_layout)
                    .saved()
                    .clone()
            });
        let draft = WorkspaceLayout::current(cx).saved().clone();
        self.workspace_edit = Some(WorkspaceEditing {
            space_id: space.id,
            space_name: space.name.clone(),
            profile_default,
            draft,
            undo: None,
        });
        self.open_menu = None;
        window.blur(cx);
        self.notify_workspace_edit(cx);
    }

    pub(super) fn cancel_workspace_edit(&mut self, cx: &mut Context<Self>) {
        self.workspace_edit = None;
        self.notify_workspace_edit(cx);
    }

    pub(super) fn workspace_layout(&self, cx: &gpui::App) -> WorkspaceLayout {
        self.workspace_edit
            .as_ref()
            .and_then(|editing| WorkspaceLayout::from_saved(&editing.draft))
            .unwrap_or_else(|| WorkspaceLayout::current(cx))
    }

    fn notify_workspace_edit(&mut self, cx: &mut Context<Self>) {
        let placement = self
            .workspace_edit
            .as_ref()
            .and_then(|editing| WorkspaceLayout::from_saved(&editing.draft))
            .map(|layout| layout.omnibox_placement());
        let _ = self.omnibox.update(cx, |omnibox, cx| {
            omnibox.set_workspace_placement(placement, cx);
        });
        let _ = self.chrome.update(cx, |_, cx| cx.notify());
        cx.notify();
    }

    fn finish_workspace_edit(&mut self, cx: &mut Context<Self>) {
        let Some(editing) = self.workspace_edit.as_ref() else {
            return;
        };
        if WorkspaceLayout::from_saved(&editing.draft).is_none() {
            return;
        }
        let (space_id, layout, follows_default) = (
            editing.space_id,
            editing.draft.clone(),
            editing.draft == editing.profile_default,
        );
        Settings::update(cx, |settings| {
            if follows_default {
                settings.spaces.clear_layout_override(space_id);
            } else {
                settings.spaces.set_layout_override(space_id, &layout);
            }
        });
        self.workspace_edit = None;
        self.notify_workspace_edit(cx);
    }

    fn undo_workspace_edit(&mut self, cx: &mut Context<Self>) {
        let Some(editing) = self.workspace_edit.as_mut() else {
            return;
        };
        let Some(previous) = editing.undo.take() else {
            return;
        };
        editing.draft = previous;
        self.notify_workspace_edit(cx);
    }

    fn reset_workspace_edit(&mut self, cx: &mut Context<Self>) {
        let Some(editing) = self.workspace_edit.as_mut() else {
            return;
        };
        if editing.draft == editing.profile_default {
            return;
        }
        editing.undo = Some(editing.draft.clone());
        editing.draft = editing.profile_default.clone();
        self.notify_workspace_edit(cx);
    }

    fn apply_workspace_preset(&mut self, layout: SavedWorkspaceLayout, cx: &mut Context<Self>) {
        let Some(editing) = self.workspace_edit.as_mut() else {
            return;
        };
        if editing.draft == layout || WorkspaceLayout::from_saved(&layout).is_none() {
            return;
        }
        editing.undo = Some(editing.draft.clone());
        editing.draft = layout;
        self.notify_workspace_edit(cx);
    }

    fn place_workspace_component(
        &mut self,
        id: &str,
        region: DockRegion,
        index: usize,
        cx: &mut Context<Self>,
    ) {
        let Some(editing) = self.workspace_edit.as_mut() else {
            return;
        };
        let mut draft = editing.draft.clone();
        if !ensure_workspace_component(&mut draft, id) {
            return;
        }
        let source_position = component_region_index(&draft, id);
        let index = if index != usize::MAX
            && source_position.is_some_and(|(source_region, source_index)| {
                source_region == region && source_index < index
            }) {
            index.saturating_sub(1)
        } else {
            index
        };
        if draft
            .component(id)
            .is_some_and(|component| !component.visible)
        {
            if !set_workspace_component_visible(&mut draft, id, true) {
                return;
            }
        }
        if !normalize_component_placement(&mut draft, id, Some(region)) {
            return;
        }
        draft.move_component(id, region, index);
        replace_exclusive_group_conflicts(&mut draft, id);
        align_side_group(&mut draft, id, Some(region));
        if draft == editing.draft || WorkspaceLayout::from_saved(&draft).is_none() {
            return;
        }
        editing.undo = Some(editing.draft.clone());
        editing.draft = draft;
        self.notify_workspace_edit(cx);
    }

    fn remove_workspace_component(&mut self, id: &str, cx: &mut Context<Self>) {
        let Some(editing) = self.workspace_edit.as_mut() else {
            return;
        };
        let mut draft = editing.draft.clone();
        if !set_workspace_component_visible(&mut draft, id, false)
            || WorkspaceLayout::from_saved(&draft).is_none()
        {
            return;
        }
        editing.undo = Some(editing.draft.clone());
        editing.draft = draft;
        self.notify_workspace_edit(cx);
    }

    pub(super) fn workspace_editor_chrome(
        &self,
        editing: &WorkspaceEditing,
        transition: super::super::motion::Transition,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let draft = editing.draft.clone();
        let layout =
            WorkspaceLayout::from_saved(&draft).unwrap_or_else(|| WorkspaceLayout::current(cx));
        let palette = self.palette(window, cx);
        let has_top_controls = components_in_region(&draft, DockRegion::Top)
            .iter()
            .any(|component| component.id != ComponentId::TabStrip.key());
        let bottom_components = components_in_region(&draft, DockRegion::Bottom);
        let has_bottom_tabs = bottom_components
            .iter()
            .any(|component| component.id == ComponentId::TabStrip.key());
        let has_bottom_controls = bottom_components
            .iter()
            .any(|component| component.id != ComponentId::TabStrip.key());
        let top_height = (layout.top_extent()
            + if has_top_controls {
                metrics::WORKSPACE_CANVAS_GAP
            } else {
                0.0
            })
        .max(metrics::TITLEBAR_HEIGHT);
        let bottom_height = (layout.bottom_extent()
            + if has_bottom_tabs && has_bottom_controls {
                metrics::WORKSPACE_CANVAS_GAP
            } else {
                0.0
            })
        .max(metrics::TITLEBAR_HEIGHT);
        let bottom_offset = metrics::WORKSPACE_LIBRARY_HEIGHT;
        let mut root = div().id("workspace-editor-chrome").size_full().relative();

        root = root.child(self.workspace_horizontal_dock(
            DockRegion::Top,
            &draft,
            top_height,
            0.0,
            palette,
            window,
            cx,
        ));
        root = root.child(self.workspace_horizontal_dock(
            DockRegion::Bottom,
            &draft,
            bottom_height,
            bottom_offset,
            palette,
            window,
            cx,
        ));

        if let Some(region) = layout.sidebar_region() {
            let width = layout
                .sidebar_extent()
                .map_or_else(|| self.sidebar.width(cx), |width| width as f32);
            root = root.child(self.workspace_side_dock(
                region,
                &draft,
                width,
                layout.top_extent() + metrics::TITLEBAR_HEIGHT,
                layout.bottom_extent() + bottom_offset,
                palette,
                window,
                cx,
            ));
            root = root.child(self.workspace_side_navigation(
                region,
                &draft,
                width,
                layout.top_extent(),
                palette,
                window,
                cx,
            ));
            let opposite = if region == DockRegion::Leading {
                DockRegion::Trailing
            } else {
                DockRegion::Leading
            };
            root = root.child(self.workspace_empty_side_dock(
                opposite,
                width,
                layout.top_extent() + metrics::TITLEBAR_HEIGHT,
                layout.bottom_extent() + bottom_offset,
                palette,
                cx,
            ));
        } else {
            root = root.child(self.workspace_empty_side_dock(
                DockRegion::Leading,
                32.0,
                layout.top_extent() + metrics::TITLEBAR_HEIGHT,
                layout.bottom_extent() + bottom_offset,
                palette,
                cx,
            ));
            root = root.child(self.workspace_empty_side_dock(
                DockRegion::Trailing,
                32.0,
                layout.top_extent() + metrics::TITLEBAR_HEIGHT,
                layout.bottom_extent() + bottom_offset,
                palette,
                cx,
            ));
        }

        root.animate(
            "workspace-editor-chrome-presence",
            Entrance::new().fade_from(0.0).speed(Speed::Standard),
            transition,
        )
        .into_any_element()
    }

    fn workspace_horizontal_dock(
        &self,
        region: DockRegion,
        draft: &SavedWorkspaceLayout,
        height: f32,
        bottom_offset: f32,
        palette: ThemeColors,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let components = components_in_region(draft, region);
        let tab_strip = components
            .iter()
            .find(|component| component.id == ComponentId::TabStrip.key());
        let controls = components
            .iter()
            .filter(|component| component.id != ComponentId::TabStrip.key())
            .collect::<Vec<_>>();
        let mut dock = v_stack()
            .id(format!("workspace-edit-dock-{}", region_key(region)))
            .absolute()
            .when(region == DockRegion::Bottom, |dock| {
                dock.bottom(px(bottom_offset))
            })
            .when(region == DockRegion::Top, |dock| dock.top_0())
            .left_0()
            .right_0()
            .h(px(height))
            .gap(px(metrics::WORKSPACE_CANVAS_GAP))
            .justify_center()
            .on_drop(
                cx.listener(move |this, dragged: &WorkspaceComponentDrag, _, cx| {
                    this.place_workspace_component(&dragged.id, region, usize::MAX, cx);
                }),
            )
            .drag_over::<WorkspaceComponentDrag>(move |style, dragged, _, _| {
                if can_place_in_region(&dragged.id, region) {
                    style
                        .border_1()
                        .border_dashed()
                        .border_color(rgb(palette.chosen))
                        .bg(rgba(palette.selection))
                } else {
                    style
                }
            });

        if tab_strip.is_some() {
            let index = component_region_index(draft, ComponentId::TabStrip.key())
                .map_or(0, |(_, index)| index);
            let component = self.workspace_editor_component(
                ComponentId::TabStrip,
                region,
                index,
                self.render_tab_strip_content(window, cx),
                false,
                palette,
                cx,
            );
            let controls_inset = if region == DockRegion::Top
                && !window.is_fullscreen()
                && !window.is_simple_fullscreen()
            {
                metrics::WINDOW_CONTROLS_INSET
            } else {
                0.0
            };
            dock = dock.child(
                h_stack()
                    .w_full()
                    .h(px(metrics::TITLEBAR_HEIGHT))
                    .items_center()
                    .px(px(controls_inset))
                    .child(component),
            );
        }
        if region == DockRegion::Top && tab_strip.is_none() && !controls.is_empty() {
            dock = dock.child(div().w_full().h(px(metrics::TITLEBAR_HEIGHT)));
        }
        if !controls.is_empty() {
            let mut row = h_stack()
                .w_full()
                .h(px(metrics::TOOLBAR_HEIGHT))
                .items_center()
                .px(px(metrics::TOOLBAR_HORIZONTAL_INSET))
                .gap(px(metrics::WORKSPACE_CANVAS_GAP));
            for component in controls.iter() {
                let id = component_id(&component.id);
                let index =
                    component_region_index(draft, &component.id).map_or(0, |(_, index)| index);
                row = row.child(self.workspace_editor_component(
                    id,
                    region,
                    index,
                    self.render_component(id, region, window, cx),
                    false,
                    palette,
                    cx,
                ));
            }
            dock = dock.child(row);
        }
        dock.into_any_element()
    }

    fn workspace_side_dock(
        &self,
        region: DockRegion,
        draft: &SavedWorkspaceLayout,
        width: f32,
        top_offset: f32,
        bottom_offset: f32,
        palette: ThemeColors,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let components = components_in_region(draft, region);
        let leading = region == DockRegion::Leading;
        let mut dock = v_stack()
            .id(format!("workspace-edit-dock-{}", region_key(region)))
            .absolute()
            .top(px(top_offset))
            .bottom(px(bottom_offset))
            .w(px(width))
            .when(leading, |dock| dock.left_0())
            .when(!leading, |dock| dock.right_0())
            .on_drop(
                cx.listener(move |this, dragged: &WorkspaceComponentDrag, _, cx| {
                    this.place_workspace_component(&dragged.id, region, usize::MAX, cx);
                }),
            )
            .drag_over::<WorkspaceComponentDrag>(move |style, dragged, _, _| {
                if can_place_in_region(&dragged.id, region) {
                    style.bg(rgba(palette.selection))
                } else {
                    style
                }
            });

        let navigation_ids = [
            ComponentId::SidebarToggle,
            ComponentId::BackButton,
            ComponentId::ForwardButton,
            ComponentId::ReloadButton,
            ComponentId::BrowserMenuButton,
        ];
        let panels = components
            .iter()
            .filter(|component| {
                !navigation_ids
                    .iter()
                    .any(|id| id.key() == component.id.as_str())
            })
            .collect::<Vec<_>>();
        let mut has_fixed_panel = false;
        let mut previous_was_fixed = false;
        for component in panels {
            let id = component_id(&component.id);
            if matches!(id, ComponentId::AddressField | ComponentId::FavouritesPanel) {
                if previous_was_fixed {
                    dock = dock.child(div().h(px(metrics::SIDEBAR_SECTION_GAP)).flex_shrink_0());
                }
                previous_was_fixed = true;
                has_fixed_panel = true;
            } else {
                previous_was_fixed = false;
            }
            if id == ComponentId::VerticalTabs && has_fixed_panel {
                dock = dock.child(
                    div()
                        .mx(px(metrics::SIDEBAR_SEPARATOR_INSET))
                        .my(px(metrics::SIDEBAR_SEPARATOR_MARGIN))
                        .h(px(metrics::MENU_SEPARATOR_HEIGHT))
                        .flex_shrink_0()
                        .bg(rgba(palette.menu_border)),
                );
            }
            let index = component_region_index(draft, &component.id).map_or(0, |(_, index)| index);
            let content = self.workspace_editor_panel_component(id, region, window, palette, cx);
            let frame =
                self.workspace_editor_component(id, region, index, content, true, palette, cx);
            dock = if matches!(id, ComponentId::AddressField | ComponentId::FavouritesPanel) {
                dock.child(
                    h_stack()
                        .w_full()
                        .px(px(metrics::SIDEBAR_PADDING))
                        .child(frame),
                )
            } else {
                dock.child(frame)
            };
        }
        dock.into_any_element()
    }

    fn workspace_side_navigation(
        &self,
        region: DockRegion,
        draft: &SavedWorkspaceLayout,
        width: f32,
        top_offset: f32,
        palette: ThemeColors,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let components = components_in_region(draft, region);
        let leading = region == DockRegion::Leading;
        let component_index =
            |id: ComponentId| component_region_index(draft, id.key()).map_or(0, |(_, index)| index);
        let toggle = components
            .iter()
            .find(|component| component.id == ComponentId::SidebarToggle.key())
            .map(|_| {
                let id = ComponentId::SidebarToggle;
                self.workspace_editor_component(
                    id,
                    region,
                    component_index(id),
                    self.render_component(id, region, window, cx),
                    false,
                    palette,
                    cx,
                )
            });
        let controls = components
            .iter()
            .filter_map(|component| {
                let id = ComponentId::from_key(&component.id)?;
                matches!(
                    id,
                    ComponentId::BackButton
                        | ComponentId::ForwardButton
                        | ComponentId::ReloadButton
                        | ComponentId::BrowserMenuButton
                )
                .then_some(id)
            })
            .map(|id| {
                self.workspace_editor_component(
                    id,
                    region,
                    component_index(id),
                    self.render_component(id, region, window, cx),
                    false,
                    palette,
                    cx,
                )
            })
            .collect::<Vec<_>>();
        if toggle.is_none() && controls.is_empty() {
            return div().into_any_element();
        }

        navigation_bar(
            width,
            1.0,
            window.is_fullscreen() || window.is_simple_fullscreen(),
            leading,
            toggle,
            controls,
        )
        .id(format!("workspace-edit-navigation-{}", region_key(region)))
        .absolute()
        .top(px(top_offset))
        .when(leading, |navigation| navigation.left_0())
        .when(!leading, |navigation| navigation.right_0())
        .into_any_element()
    }

    fn workspace_empty_side_dock(
        &self,
        region: DockRegion,
        width: f32,
        top_offset: f32,
        bottom_offset: f32,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let leading = region == DockRegion::Leading;
        let dock = div()
            .id(format!("workspace-edit-dock-{}", region_key(region)))
            .absolute()
            .top(px(top_offset))
            .bottom(px(bottom_offset))
            .w(px(width.min(metrics::WORKSPACE_DOCK_SIDE_WIDTH)))
            .when(leading, |dock| dock.left_0())
            .when(!leading, |dock| dock.right_0())
            .on_drop(
                cx.listener(move |this, dragged: &WorkspaceComponentDrag, _, cx| {
                    this.place_workspace_component(&dragged.id, region, usize::MAX, cx);
                }),
            )
            .drag_over::<WorkspaceComponentDrag>(move |style, dragged, _, _| {
                if can_place_in_region(&dragged.id, region) {
                    style
                        .border_1()
                        .border_dashed()
                        .border_color(rgb(palette.chosen))
                        .bg(rgba(palette.selection))
                } else {
                    style
                }
            });
        dock.into_any_element()
    }

    fn workspace_editor_panel_component(
        &self,
        component: ComponentId,
        region: DockRegion,
        window: &Window,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        match component {
            ComponentId::AddressField => div()
                .w_full()
                .child(self.omnibox.clone())
                .into_any_element(),
            ComponentId::FavouritesPanel => {
                let settings = Settings::get(cx).sidebar.clone();
                let favourites = self.favourite_tiles(&settings.favourites, palette, cx);
                if favourites.is_empty() {
                    v_stack()
                        .w_full()
                        .h(px(metrics::WORKSPACE_DOCK_MIN_HEIGHT))
                        .items_center()
                        .justify_center()
                        .text_color(rgb(palette.text_secondary))
                        .child("Favourites")
                        .into_any_element()
                } else {
                    super::super::sidebar::favourites_grid(favourites, palette).into_any_element()
                }
            }
            ComponentId::VerticalTabs => v_stack()
                .id("workspace-edit-sidebar-tabs")
                .size_full()
                .min_h_0()
                .overflow_y_scroll()
                .px(px(metrics::SIDEBAR_PADDING))
                .child(self.sidebar_tabs(palette, cx))
                .into_any_element(),
            ComponentId::SidebarFooter => div()
                .w_full()
                .child(self.sidebar_footer(palette, cx))
                .into_any_element(),
            ComponentId::TabStrip => self.render_component(component, region, window, cx),
            _ => self.render_component(component, region, window, cx),
        }
    }

    fn workspace_editor_component(
        &self,
        component: ComponentId,
        region: DockRegion,
        index: usize,
        content: AnyElement,
        full_width: bool,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> AnyElement {
        let definition = component_definition(component);
        let id = component.key().to_owned();
        let drag_label = SharedString::from(definition.name);
        let drag_id = id.clone();
        let preview = div()
            .id(format!("workspace-component-handle-{id}"))
            .role(Role::Button)
            .aria_label(format!("Drag {}", definition.name))
            .tab_index(0)
            .absolute()
            .top(px(-metrics::WORKSPACE_DRAG_HANDLE_SIZE / 2.0))
            .left(px(-metrics::WORKSPACE_DRAG_HANDLE_SIZE / 2.0))
            .size(px(metrics::WORKSPACE_DRAG_HANDLE_SIZE))
            .items_center()
            .justify_center()
            .rounded(px(metrics::CONTROL_RADIUS))
            .border_1()
            .border_color(rgba(palette.menu_border))
            .raised(palette)
            .text_color(rgb(palette.text_primary))
            .child("⠿")
            .on_drag(
                WorkspaceComponentDrag { id: drag_id },
                move |_, _, _, cx| {
                    let label = drag_label.clone();
                    cx.new(|_| WorkspaceDragPreview { label, palette })
                },
            );
        let frame = div()
            .id(format!("workspace-component-frame-{id}"))
            .relative()
            .when(full_width, |frame| frame.w_full().min_h_0())
            .when(component == ComponentId::VerticalTabs, |frame| {
                frame.flex_1().min_h_0()
            })
            .when(
                component == ComponentId::AddressField && !full_width,
                |frame| frame.flex_1().min_w_0(),
            )
            .when(
                component != ComponentId::AddressField && !full_width,
                |frame| frame.flex_shrink_0(),
            )
            .rounded(px(metrics::CONTROL_RADIUS))
            .border_1()
            .border_color(rgba(palette.menu_border))
            .child(content)
            .child(
                div()
                    .absolute()
                    .inset_0()
                    .occlude()
                    .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
                    .on_mouse_down(MouseButton::Right, |_, _, cx| cx.stop_propagation()),
            )
            .child(preview)
            .on_drop(
                cx.listener(move |this, dragged: &WorkspaceComponentDrag, _, cx| {
                    if can_place_in_region(&dragged.id, region) {
                        this.place_workspace_component(&dragged.id, region, index, cx);
                    }
                }),
            )
            .drag_over::<WorkspaceComponentDrag>(move |style, dragged, _, _| {
                if can_place_in_region(&dragged.id, region) {
                    style.border_color(rgb(palette.chosen))
                } else {
                    style
                }
            });
        frame.into_any_element()
    }

    pub(super) fn workspace_editor_overlay(
        &mut self,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let (editing, transition) = self.workspace_edit_presence.sync(
            self.workspace_edit.clone(),
            Entrance::new()
                .fade_from(0.0)
                .slide_from(Edge::Bottom, distance::NUDGE)
                .speed(Speed::Standard),
            cx,
        )?;
        Some(
            self.workspace_editor_panel(editing, palette, transition, cx)
                .into_any_element(),
        )
    }

    fn workspace_editor_panel(
        &self,
        editing: WorkspaceEditing,
        palette: ThemeColors,
        transition: super::super::motion::Transition,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + 'static + use<> {
        let undo_enabled = editing.undo.is_some();
        let reset_enabled = editing.draft != editing.profile_default;
        let horizontal_selected = editing.draft == SavedWorkspaceLayout::horizontal();
        let vertical_selected = editing.draft == SavedWorkspaceLayout::vertical();
        let header = h_stack()
            .w_full()
            .items_center()
            .justify_between()
            .gap(px(metrics::WORKSPACE_CANVAS_GAP))
            .child(
                v_stack()
                    .gap(px(metrics::MENU_ITEM_GAP))
                    .child(
                        div()
                            .text_size(px(metrics::INTERNAL_PAGE_BODY_SIZE))
                            .child(format!("Edit {}'s workspace", editing.space_name)),
                    )
                    .child(
                        div()
                            .text_size(px(metrics::TAB_FONT_SIZE))
                            .text_color(rgb(palette.text_secondary))
                            .child("Drag components to move them; drop one in the library to remove it."),
                    ),
            )
            .child(
                h_stack()
                    .items_center()
                    .gap(px(metrics::BUTTON_GAP))
                    .child(editor_button(
                        "workspace-edit-horizontal-preset".to_owned(),
                        if horizontal_selected {
                            "Horizontal ✓".into()
                        } else {
                            "Horizontal".into()
                        },
                        "Apply the horizontal workspace preset".into(),
                        true,
                        horizontal_selected,
                        palette,
                        Box::new(cx.listener(|this, _, _, cx| {
                            this.apply_workspace_preset(SavedWorkspaceLayout::horizontal(), cx)
                        })),
                    ))
                    .child(editor_button(
                        "workspace-edit-vertical-preset".to_owned(),
                        if vertical_selected {
                            "Vertical ✓".into()
                        } else {
                            "Vertical".into()
                        },
                        "Apply the vertical workspace preset".into(),
                        true,
                        vertical_selected,
                        palette,
                        Box::new(cx.listener(|this, _, _, cx| {
                            this.apply_workspace_preset(SavedWorkspaceLayout::vertical(), cx)
                        })),
                    )),
            )
            .child(
                h_stack()
                    .items_center()
                    .gap(px(metrics::BUTTON_GAP))
                    .child(editor_button(
                        "workspace-edit-reset".to_owned(),
                        "Reset".into(),
                        "Restore the default workspace layout".into(),
                        reset_enabled,
                        false,
                        palette,
                        Box::new(cx.listener(|this, _, _, cx| this.reset_workspace_edit(cx))),
                    ))
                    .child(editor_button(
                        "workspace-edit-undo".to_owned(),
                        "Undo".into(),
                        "Undo the last layout change".into(),
                        undo_enabled,
                        false,
                        palette,
                        Box::new(cx.listener(|this, _, _, cx| this.undo_workspace_edit(cx))),
                    ))
                    .child(editor_button(
                        "workspace-edit-cancel".to_owned(),
                        "Cancel".into(),
                        "Discard workspace changes".into(),
                        true,
                        false,
                        palette,
                        Box::new(cx.listener(|this, _, _, cx| this.cancel_workspace_edit(cx))),
                    ))
                    .child(editor_button(
                        "workspace-edit-done".to_owned(),
                        "Done".into(),
                        "Save workspace changes for this space".into(),
                        WorkspaceLayout::from_saved(&editing.draft).is_some(),
                        true,
                        palette,
                        Box::new(cx.listener(|this, _, _, cx| this.finish_workspace_edit(cx))),
                    )),
            );

        v_stack()
            .id("workspace-editor-overlay")
            .absolute()
            .inset_0()
            .justify_end()
            .child(
                v_stack()
                    .id("workspace-editor-library-surface")
                    .w_full()
                    .h(px(metrics::WORKSPACE_LIBRARY_HEIGHT))
                    .gap(px(metrics::WORKSPACE_LIBRARY_GAP))
                    .p(px(metrics::WORKSPACE_LIBRARY_PADDING))
                    .border_t_1()
                    .border_color(rgba(palette.menu_border))
                    .raised(palette)
                    .elevated(Elevation::Medium)
                    .text_color(rgb(palette.text_primary))
                    .child(header)
                    .child(self.workspace_library(&editing.draft, palette, cx)),
            )
            .animate(
                ElementId::Name("workspace-editor-library-enter-exit".into()),
                Entrance::new()
                    .fade_from(0.0)
                    .slide_from(Edge::Bottom, distance::NUDGE)
                    .speed(Speed::Standard),
                transition,
            )
    }

    fn workspace_library(
        &self,
        draft: &SavedWorkspaceLayout,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement + use<> {
        let hidden = component_catalog()
            .iter()
            .filter(|definition| {
                draft
                    .component(definition.id.key())
                    .is_none_or(|component| !component.visible)
            })
            .map(|definition| {
                let id = definition.id.key().to_owned();
                let drag_label = SharedString::from(definition.name);
                let label = drag_label.clone();
                h_stack()
                    .id(format!("workspace-library-{}", definition.id.key()))
                    .role(Role::Button)
                    .aria_label(format!(
                        "Drag {} into the workspace. {}",
                        definition.name, definition.description
                    ))
                    .tab_index(0)
                    .items_center()
                    .flex_shrink_0()
                    .gap(px(metrics::MENU_ITEM_GAP))
                    .h(px(metrics::BUTTON_HEIGHT))
                    .px(px(metrics::BUTTON_HORIZONTAL_PADDING))
                    .rounded(px(metrics::CONTROL_RADIUS))
                    .border_1()
                    .border_color(rgba(palette.menu_border))
                    .raised(palette)
                    .text_color(rgb(palette.text_primary))
                    .child("⠿")
                    .child(definition.name)
                    .on_drag(WorkspaceComponentDrag { id }, move |_, _, _, cx| {
                        let label = label.clone();
                        cx.new(|_| WorkspaceDragPreview { label, palette })
                    })
                    .into_any_element()
            })
            .collect::<Vec<_>>();
        let is_empty = hidden.is_empty();
        let layout_for_drop = draft.clone();
        let mut library = h_stack()
            .id("workspace-component-library")
            .role(Role::Group)
            .aria_label("Component library")
            .w_full()
            .flex_1()
            .min_h_0()
            .gap(px(metrics::WORKSPACE_LIBRARY_GAP))
            .items_center()
            .flex_wrap()
            .overflow_x_scroll()
            .children(hidden);
        if is_empty {
            library = library.child(
                div()
                    .text_size(px(metrics::TAB_FONT_SIZE))
                    .text_color(rgb(palette.text_secondary))
                    .child("All components are in the workspace · drop here to remove one"),
            );
        }
        library
            .on_drop(
                cx.listener(|this, dragged: &WorkspaceComponentDrag, _, cx| {
                    this.remove_workspace_component(&dragged.id, cx);
                }),
            )
            .drag_over::<WorkspaceComponentDrag>(move |style, dragged, _, _| {
                if can_set_workspace_component_visible(&layout_for_drop, &dragged.id, false) {
                    style
                        .border_1()
                        .border_dashed()
                        .border_color(rgb(palette.chosen))
                        .bg(rgba(palette.selection))
                } else {
                    style
                }
            })
    }
}

/// Brings hidden or older component data into the current shell catalog before
/// making it visible. An explicit region is used for a drag or move action;
/// otherwise a supported saved region is retained.
fn normalize_component_placement(
    layout: &mut SavedWorkspaceLayout,
    id: &str,
    requested_region: Option<DockRegion>,
) -> bool {
    let Some(component_id) = ComponentId::from_key(id) else {
        return false;
    };
    let definition = component_definition(component_id);
    let Some(component) = layout.component(id) else {
        return false;
    };
    let region = requested_region.unwrap_or_else(|| {
        if definition.supported_regions.contains(&component.region) {
            component.region
        } else {
            definition.default_region
        }
    });
    if !definition.supported_regions.contains(&region) {
        return false;
    }
    if component.region != region {
        layout.move_component(id, region, usize::MAX);
    }
    let extent = layout
        .component(id)
        .and_then(|component| component.preferred_extent);
    if let Some(bounds) = definition.preferred_extent {
        if extent.is_none_or(|extent| extent < bounds.minimum || extent > bounds.maximum) {
            layout.set_preferred_extent(id, Some(bounds.default));
        }
    } else if extent.is_some() {
        layout.set_preferred_extent(id, None);
    }
    true
}

fn ensure_workspace_component(layout: &mut SavedWorkspaceLayout, id: &str) -> bool {
    if layout.component(id).is_some() {
        return true;
    }
    let Some(component_id) = ComponentId::from_key(id) else {
        return false;
    };
    let definition = component_definition(component_id);
    layout.add_component(id, definition.default_region, false)
}

fn can_set_workspace_component_visible(
    layout: &SavedWorkspaceLayout,
    id: &str,
    visible: bool,
) -> bool {
    let mut candidate = layout.clone();
    set_workspace_component_visible(&mut candidate, id, visible)
}

/// Applies catalog group rules whenever a component is added or removed.
/// Components in a shared exclusive group replace one another, so a layout
/// can never expose both horizontal and vertical tabs.
fn set_workspace_component_visible(
    layout: &mut SavedWorkspaceLayout,
    id: &str,
    visible: bool,
) -> bool {
    let Some(component_id) = ComponentId::from_key(id) else {
        return false;
    };
    let mut candidate = layout.clone();
    if !ensure_workspace_component(&mut candidate, id) {
        return false;
    }
    if candidate
        .component(id)
        .is_some_and(|component| component.visible == visible)
    {
        return false;
    }

    if !candidate.set_component_visible(id, visible) {
        return false;
    }
    if visible {
        replace_exclusive_group_conflicts(&mut candidate, id);
        align_side_group(&mut candidate, id, None);
    }

    // Required catalog groups regain a compatible visible member after edits.
    for (group, minimum_visible, _) in component_groups() {
        let count = visible_group_count(&candidate, group);
        if count >= minimum_visible {
            continue;
        }
        let replacement = component_catalog()
            .iter()
            .filter(|definition| {
                definition.id != component_id && definition.groups.contains(&group)
            })
            .min_by_key(|definition| group_conflict_count(&candidate, definition));
        let Some(replacement) = replacement else {
            return false;
        };
        if group_conflict_count(&candidate, replacement) != 0 {
            return false;
        }
        if !ensure_workspace_component(&mut candidate, replacement.id.key())
            || !candidate.set_component_visible(replacement.id.key(), true)
        {
            return false;
        }
    }

    if WorkspaceLayout::from_saved(&candidate).is_none() {
        return false;
    }
    *layout = candidate;
    true
}

/// Replaces only members of catalog groups that are explicitly exclusive.
fn replace_exclusive_group_conflicts(layout: &mut SavedWorkspaceLayout, id: &str) {
    let Some(component_id) = ComponentId::from_key(id) else {
        return;
    };
    let definition = component_definition(component_id);
    for other in component_catalog() {
        if other.id == component_id
            || !other.groups.iter().any(|group| {
                definition.groups.contains(group)
                    && component_group_bounds(group).is_some_and(|(_, maximum)| maximum <= 1)
            })
        {
            continue;
        }
        if layout
            .component(other.id.key())
            .is_some_and(|component| component.visible)
        {
            layout.set_component_visible(other.id.key(), false);
        }
    }
}

/// Visible side panels form one dock. Moving or adding one to the other side
/// carries its side-dock siblings with it.
fn align_side_group(
    layout: &mut SavedWorkspaceLayout,
    id: &str,
    requested_region: Option<DockRegion>,
) {
    let Some(component_id) = ComponentId::from_key(id) else {
        return;
    };
    let definition = component_definition(component_id);
    let Some(group) = definition
        .groups
        .iter()
        .find(|group| component_group_keeps_side_members_together(group))
    else {
        return;
    };
    if requested_region
        .is_some_and(|region| !matches!(region, DockRegion::Leading | DockRegion::Trailing))
    {
        return;
    }
    let target = requested_region
        .filter(|region| matches!(region, DockRegion::Leading | DockRegion::Trailing))
        .or_else(|| {
            layout
                .components
                .iter()
                .filter(|component| component.visible && component.id != id)
                .filter(|component| {
                    ComponentId::from_key(&component.id)
                        .is_some_and(|other| component_definition(other).groups.contains(group))
                })
                .map(|component| component.region)
                .find(|region| matches!(region, DockRegion::Leading | DockRegion::Trailing))
        })
        .or_else(|| {
            layout
                .component(id)
                .map(|component| component.region)
                .filter(|region| matches!(region, DockRegion::Leading | DockRegion::Trailing))
        });
    let Some(target) = target else {
        return;
    };
    if layout.component(id).is_some_and(|component| {
        matches!(component.region, DockRegion::Leading | DockRegion::Trailing)
            && component.region != target
    }) {
        layout.move_component(id, target, usize::MAX);
    }
    for other in component_catalog()
        .iter()
        .filter(|other| other.id != component_id && other.groups.contains(group))
    {
        let Some(component) = layout.component(other.id.key()) else {
            continue;
        };
        if component.visible
            && matches!(component.region, DockRegion::Leading | DockRegion::Trailing)
            && component.region != target
        {
            layout.move_component(other.id.key(), target, usize::MAX);
        }
    }
}

fn visible_group_count(layout: &SavedWorkspaceLayout, group: &str) -> usize {
    component_catalog()
        .iter()
        .filter(|definition| {
            definition.groups.contains(&group)
                && layout
                    .component(definition.id.key())
                    .is_some_and(|component| component.visible)
        })
        .count()
}

fn group_conflict_count(layout: &SavedWorkspaceLayout, candidate: &ComponentDefinition) -> usize {
    component_catalog()
        .iter()
        .filter(|visible| {
            visible.id != candidate.id
                && layout
                    .component(visible.id.key())
                    .is_some_and(|component| component.visible)
                && candidate.groups.iter().any(|group| {
                    visible.groups.contains(group)
                        && component_group_bounds(group).is_some_and(|(_, maximum)| maximum <= 1)
                })
        })
        .count()
}

fn editor_button(
    id: String,
    label: SharedString,
    aria_label: SharedString,
    enabled: bool,
    primary: bool,
    palette: ThemeColors,
    on_click: ClickHandler,
) -> AnyElement {
    let button = h_stack()
        .id(id)
        .role(Role::Button)
        .aria_label(aria_label)
        .aria_disabled(!enabled)
        .tab_index(0)
        .focus_visible(|style| style.border_1().border_color(rgb(palette.chosen)))
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .h(px(metrics::SMALL_BUTTON_HEIGHT))
        .px(px(metrics::SMALL_BUTTON_HORIZONTAL_PADDING))
        .rounded(px(metrics::CONTROL_RADIUS))
        .text_size(px(metrics::SMALL_BUTTON_FONT_SIZE))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .child(label);
    let button = if primary {
        button
            .bg(rgba(palette.selection))
            .text_color(rgb(palette.on_chosen))
    } else {
        button
            .text_color(rgb(palette.text_primary))
            .bg(rgba(palette.surface))
            .hover(|style| style.bg(rgba(palette.selected_surface)))
    };
    if enabled {
        button.on_click(on_click).into_any_element()
    } else {
        button.opacity(metrics::DISABLED_OPACITY).into_any_element()
    }
}

fn components_in_region(
    layout: &SavedWorkspaceLayout,
    region: DockRegion,
) -> Vec<&LayoutComponent> {
    let mut components = layout
        .components
        .iter()
        .filter(|component| component.visible && component.region == region)
        .collect::<Vec<_>>();
    components.sort_by_key(|component| component.order);
    components
}

fn component_region_index(layout: &SavedWorkspaceLayout, id: &str) -> Option<(DockRegion, usize)> {
    let region = layout.component(id)?.region;
    let mut components = layout
        .components
        .iter()
        .filter(|component| component.region == region)
        .collect::<Vec<_>>();
    components.sort_by_key(|component| component.order);
    components
        .iter()
        .position(|component| component.id == id)
        .map(|index| (region, index))
}

fn can_place_in_region(id: &str, region: DockRegion) -> bool {
    ComponentId::from_key(id)
        .is_some_and(|id| component_definition(id).supported_regions.contains(&region))
}

fn component_id(id: &str) -> ComponentId {
    ComponentId::from_key(id).expect("only shell catalog components are rendered")
}

const fn region_key(region: DockRegion) -> &'static str {
    match region {
        DockRegion::Top => "top",
        DockRegion::Bottom => "bottom",
        DockRegion::Leading => "left",
        DockRegion::Trailing => "right",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use photon_core::{
        ADDRESS_FIELD_COMPONENT_ID, BROWSER_MENU_BUTTON_COMPONENT_ID, SIDEBAR_FOOTER_COMPONENT_ID,
        SIDEBAR_TOGGLE_COMPONENT_ID, TAB_STRIP_COMPONENT_ID, VERTICAL_TABS_COMPONENT_ID,
    };

    #[test]
    fn switching_to_vertical_tabs_replaces_the_horizontal_surface() {
        let mut layout = SavedWorkspaceLayout::horizontal();

        assert!(set_workspace_component_visible(
            &mut layout,
            VERTICAL_TABS_COMPONENT_ID,
            true
        ));
        assert!(!layout.component(TAB_STRIP_COMPONENT_ID).unwrap().visible);
        assert!(
            layout
                .component(VERTICAL_TABS_COMPONENT_ID)
                .unwrap()
                .visible
        );
        assert!(
            layout
                .component(ADDRESS_FIELD_COMPONENT_ID)
                .unwrap()
                .visible
        );
        assert!(WorkspaceLayout::from_saved(&layout).is_some());
    }

    #[test]
    fn adding_horizontal_tabs_hides_vertical_tabs_without_hiding_other_panels() {
        let mut layout = SavedWorkspaceLayout::vertical();

        assert!(set_workspace_component_visible(
            &mut layout,
            TAB_STRIP_COMPONENT_ID,
            true
        ));
        assert!(layout.component(TAB_STRIP_COMPONENT_ID).unwrap().visible);
        assert!(
            !layout
                .component(VERTICAL_TABS_COMPONENT_ID)
                .unwrap()
                .visible
        );
        assert!(
            layout
                .component(ADDRESS_FIELD_COMPONENT_ID)
                .unwrap()
                .visible
        );
        assert!(WorkspaceLayout::from_saved(&layout).is_some());
    }

    #[test]
    fn moving_a_side_panel_moves_the_side_dock_as_a_unit() {
        let mut layout = SavedWorkspaceLayout::vertical();
        assert!(layout.move_component(VERTICAL_TABS_COMPONENT_ID, DockRegion::Trailing, 0));
        align_side_group(
            &mut layout,
            VERTICAL_TABS_COMPONENT_ID,
            Some(DockRegion::Trailing),
        );
        assert_eq!(
            layout
                .component(SIDEBAR_TOGGLE_COMPONENT_ID)
                .unwrap()
                .region,
            DockRegion::Trailing
        );
        assert_eq!(
            layout.component(ADDRESS_FIELD_COMPONENT_ID).unwrap().region,
            DockRegion::Trailing
        );
        assert!(WorkspaceLayout::from_saved(&layout).is_some());
    }

    #[test]
    fn moving_the_address_field_out_of_the_sidebar_leaves_other_panels_in_place() {
        let mut layout = SavedWorkspaceLayout::vertical();
        assert!(normalize_component_placement(
            &mut layout,
            ADDRESS_FIELD_COMPONENT_ID,
            Some(DockRegion::Top)
        ));
        align_side_group(
            &mut layout,
            ADDRESS_FIELD_COMPONENT_ID,
            Some(DockRegion::Top),
        );

        assert_eq!(
            layout.component(ADDRESS_FIELD_COMPONENT_ID).unwrap().region,
            DockRegion::Top
        );
        assert!(
            layout
                .component(VERTICAL_TABS_COMPONENT_ID)
                .unwrap()
                .visible
        );
        assert_eq!(
            layout.component(VERTICAL_TABS_COMPONENT_ID).unwrap().region,
            DockRegion::Leading
        );
        assert!(WorkspaceLayout::from_saved(&layout).is_some());
    }

    #[test]
    fn every_browser_component_can_be_removed_while_the_page_remains() {
        for mut layout in [
            SavedWorkspaceLayout::horizontal(),
            SavedWorkspaceLayout::vertical(),
        ] {
            for definition in component_catalog() {
                if layout
                    .component(definition.id.key())
                    .is_some_and(|component| component.visible)
                {
                    assert!(set_workspace_component_visible(
                        &mut layout,
                        definition.id.key(),
                        false
                    ));
                }
            }

            assert!(layout.components.iter().all(|component| !component.visible));
            assert!(WorkspaceLayout::from_saved(&layout).is_some());
        }
    }

    #[test]
    fn optional_controls_and_panels_can_be_removed_independently() {
        let mut layout = SavedWorkspaceLayout::horizontal();
        assert!(set_workspace_component_visible(
            &mut layout,
            BROWSER_MENU_BUTTON_COMPONENT_ID,
            false
        ));
        assert!(set_workspace_component_visible(
            &mut layout,
            SIDEBAR_FOOTER_COMPONENT_ID,
            true
        ));
        assert!(
            !layout
                .component(BROWSER_MENU_BUTTON_COMPONENT_ID)
                .unwrap()
                .visible
        );
        assert!(
            layout
                .component(SIDEBAR_FOOTER_COMPONENT_ID)
                .unwrap()
                .visible
        );
        assert!(WorkspaceLayout::from_saved(&layout).is_some());
    }

    #[test]
    fn adding_a_side_panel_does_not_enable_the_sidebar_toggle() {
        let mut layout = SavedWorkspaceLayout::horizontal();

        assert!(set_workspace_component_visible(
            &mut layout,
            photon_core::FAVOURITES_PANEL_COMPONENT_ID,
            true
        ));

        assert!(
            layout
                .component(photon_core::FAVOURITES_PANEL_COMPONENT_ID)
                .unwrap()
                .visible
        );
        assert!(
            !layout
                .component(SIDEBAR_TOGGLE_COMPONENT_ID)
                .unwrap()
                .visible
        );
        assert!(WorkspaceLayout::from_saved(&layout).is_some());
    }
}
