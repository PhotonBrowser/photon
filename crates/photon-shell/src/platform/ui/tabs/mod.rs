//! Browser tabs, shared by the horizontal tab strip and the sidebar's
//! vertical list: a tab's model, its interactions, icon and close button.
//!
//! - `strip`: the horizontal tab strip in the titlebar.
//! - `list`: the vertical tab list in the sidebar.

mod list;
mod strip;

use gpui::{
    AnyElement, AnyWindowHandle, App, Bounds, Context, DispatchPhase, ElementId, FocusHandle,
    Hitbox, ImageSource, MouseButton, MouseDownEvent, MouseMoveEvent, ObjectFit, Render, Role,
    SharedString, SpringConfig, Stateful, Window, div, img, prelude::*, px, rgb, rgba,
};

use super::Favicon;
use super::controls::FocusRing;
use super::controls::{OwnsClicks, Pressable};
use super::icons::{
    audio_icon, close_icon, globe_icon, loading_spinner, photon_logo, sleeping_icon,
};
use super::layout::h_stack;
use super::motion::SurfaceFade;
use super::motion::{
    AnimateIn, AnimateSpring, Entrance, ReorderAxis, ReorderGroup, SpringSpec, pop,
};
use super::{
    ClickHandler, KeyDownHandler, MouseDownHandler, MouseUpHandler, metrics, theme::ThemeColors,
};
pub(super) use list::tab_list;
use std::rc::Rc;
pub(super) use strip::tab_strip;

pub(super) type TabPointerDown =
    Box<dyn Fn(&MouseDownEvent, DispatchPhase, &Hitbox, TabDragAxis, &mut Window, &mut App)>;
pub(super) type TabDragMove =
    Rc<dyn Fn(&MouseMoveEvent, DispatchPhase, &Hitbox, &mut Window, &mut App)>;
pub(super) type TabPrepaint = Box<dyn Fn(Bounds<gpui::Pixels>, &mut Window, &mut App)>;
pub(super) type TabDrop = Rc<dyn Fn(&DraggedTab, &mut Window, &mut App)>;

pub(super) fn is_foreign_tab_drag(value: &dyn std::any::Any, window: &Window) -> bool {
    value
        .downcast_ref::<DraggedTab>()
        .is_some_and(|dragged| dragged.source != window.window_handle())
}

/// Generic source and payload envelope for a native GPUI drag. Future external
/// targets can add their own payload type without changing the drag lifecycle.
/// The current tab payload only serves Photon-owned drop targets.
#[derive(Clone)]
pub(super) struct NativeDragHandoff<P> {
    pub source: AnyWindowHandle,
    pub payload: P,
}

impl<P> std::ops::Deref for NativeDragHandoff<P> {
    type Target = P;

    fn deref(&self) -> &Self::Target {
        &self.payload
    }
}

/// A stable tab payload shared by both tab layouts and Photon drop targets.
#[derive(Clone)]
pub(super) struct TabDragPayload {
    pub id: u64,
    pub size: f32,
    pub pinned: bool,
}

pub(super) type DraggedTab = NativeDragHandoff<TabDragPayload>;

/// An invisible GPUI drag view. The source tab itself dims during a drag.
pub(super) struct TabDragPreview;

impl Render for TabDragPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div().opacity(0.0)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum TabDragAxis {
    Horizontal,
    Vertical,
}

/// Separate measured layouts keep pinned tabs from snapping across the pinned
/// boundary and keep each orientation's slots in its own viewport.
#[derive(Clone)]
pub(super) struct TabReorderGroups {
    horizontal_pinned: ReorderGroup<u64>,
    horizontal_today: ReorderGroup<u64>,
    vertical_pinned: ReorderGroup<u64>,
    vertical_today: ReorderGroup<u64>,
}

impl TabReorderGroups {
    pub(super) fn new() -> Self {
        Self {
            horizontal_pinned: ReorderGroup::new(Vec::new(), ReorderAxis::X)
                .expect("empty tab order is unique"),
            horizontal_today: ReorderGroup::new(Vec::new(), ReorderAxis::X)
                .expect("empty tab order is unique"),
            vertical_pinned: ReorderGroup::new(Vec::new(), ReorderAxis::Y)
                .expect("empty tab order is unique"),
            vertical_today: ReorderGroup::new(Vec::new(), ReorderAxis::Y)
                .expect("empty tab order is unique"),
        }
    }

    pub(super) fn group(&self, axis: TabDragAxis, pinned: bool) -> &ReorderGroup<u64> {
        match (axis, pinned) {
            (TabDragAxis::Horizontal, true) => &self.horizontal_pinned,
            (TabDragAxis::Horizontal, false) => &self.horizontal_today,
            (TabDragAxis::Vertical, true) => &self.vertical_pinned,
            (TabDragAxis::Vertical, false) => &self.vertical_today,
        }
    }
}

/// What a tab shows before its title.
#[derive(Clone)]
pub(super) enum TabIcon {
    /// The page is loading; the value is the spinner's animation step.
    Loading(usize),
    Favicon(Favicon),
    /// A page is playing audio; clicking this icon toggles its mute state.
    Audio {
        favicon: Option<Favicon>,
        muted: bool,
    },
    /// A page without an icon of its own.
    Page,
    /// A page with its live Engine view retained while rendering is suppressed.
    Sleeping,
    /// The Photon logo, for Photon's own pages that use it.
    Logo,
    /// A monochrome symbol, drawn in the tab's text color at the given size.
    Symbol(fn(u32, f32) -> AnyElement),
}

/// Identifies an icon a tab reveals with the appear animation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RevealedIcon {
    Logo,
    Favicon(u64),
}

impl TabIcon {
    /// The icon the appear animation reveals. The spinner and the generic
    /// page icon are placeholders, so they neither animate nor replace the
    /// last revealed icon: a page's icon that returns after them stays put.
    pub(super) fn revealed(&self) -> Option<RevealedIcon> {
        match self {
            Self::Logo => Some(RevealedIcon::Logo),
            Self::Favicon(favicon) => Some(RevealedIcon::Favicon(favicon.key)),
            Self::Audio {
                favicon: Some(favicon),
                ..
            } => Some(RevealedIcon::Favicon(favicon.key)),
            Self::Loading(_)
            | Self::Page
            | Self::Sleeping
            | Self::Symbol(_)
            | Self::Audio { favicon: None, .. } => None,
        }
    }
}

/// How a tab icon appears.
pub(super) const ICON_ENTRANCE: Entrance = pop(metrics::TAB_FAVICON_SIZE);

/// New rows settle quickly so the list clears space as they arrive.
const TAB_ENTRY_SPRING: SpringConfig = SpringConfig::new(7_200.0, 169.71, 1.0);
const TAB_MOTION_EPSILON: f32 = 0.001;

#[derive(Clone)]
pub(super) struct TabMotion {
    pub id: ElementId,
    pub from: f32,
    pub target: f32,
    pub opacity: f32,
    pub offset_scale: f32,
}

pub(super) fn animate_tab_motion<E>(
    element: E,
    motion: Option<TabMotion>,
    axis: TabDragAxis,
    position_offset: f32,
) -> AnyElement
where
    E: Styled + IntoElement + 'static,
{
    let Some(motion) = motion else {
        return element.into_any_element();
    };
    element
        .animate_spring(
            motion.id,
            motion.from,
            motion.target,
            SpringSpec {
                config: TAB_ENTRY_SPRING,
                epsilon: TAB_MOTION_EPSILON,
            },
            move |element, value| {
                let progress = value.clamp(0.0, 1.0);
                let leaving = 1.0 - progress;
                let offset = -metrics::TAB_ENTRY_OFFSET * leaving * motion.offset_scale;
                element
                    .opacity(motion.opacity * progress)
                    .when(axis == TabDragAxis::Horizontal, |element| {
                        element.left(px(position_offset + offset))
                    })
                    .when(axis == TabDragAxis::Vertical, |element| {
                        element.top(px(position_offset + offset))
                    })
            },
        )
        .into_any_element()
}

/// Sizes an icon image, growing it in from a smaller size while
/// `appearing`, inside a fixed slot so the tab title does not move.
fn sized_icon<E>(
    icon: E,
    id: ElementId,
    appearing: bool,
    size: f32,
    animations: &super::motion::MotionConfig,
) -> AnyElement
where
    E: Styled + IntoElement + 'static,
{
    if !appearing {
        return icon.size(px(size)).flex_shrink_0().into_any_element();
    }
    div()
        .flex()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .size(px(size))
        .child(icon.animate_in(id, ICON_ENTRANCE, animations))
        .into_any_element()
}

pub(super) struct TabItem {
    pub id: SharedString,
    pub reorder_key: u64,
    /// The tab's entrance motion.
    pub motion: Option<TabMotion>,
    /// Prevents the reorder group from adding a second position animation
    /// while a measured tab insertion transition is in progress.
    pub reorder_transition: gpui_motion::Transition,
    pub label: SharedString,
    pub icon: TabIcon,
    /// Whether the icon is still growing in after it first appeared.
    pub icon_appearing: bool,
    pub animations: super::motion::MotionConfig,
    pub sleeping: bool,
    /// Whether the tab is pinned above today's tabs.
    pub pinned: bool,
    pub active: bool,
    pub focus_handle: FocusHandle,
    pub on_select: ClickHandler,
    pub on_key_down: KeyDownHandler,
    pub on_close: ClickHandler,
    pub on_toggle_audio: ClickHandler,
    pub on_pointer_down: TabPointerDown,
    pub on_prepaint: TabPrepaint,
    pub on_drop: TabDrop,
    pub position_offset: f32,
    pub dragging: bool,
    pub suppress_tooltips: bool,
    pub insertion_before: Option<f32>,
    pub insertion_after: Option<f32>,
    /// Middle-click closes the tab.
    pub on_middle_click: MouseUpHandler,
    /// Right-click opens the tab's menu.
    pub on_context_menu: MouseDownHandler,
}

/// A tab's parts, ready for the strip or the list to lay out.
struct TabParts {
    /// The tab itself, with its role, focus and interactions, but no layout.
    tab: gpui_motion::PressScaleElement<Stateful<gpui::Div>>,
    icon: AnyElement,
    label: SharedString,
    close: gpui_motion::PressScaleElement<Stateful<gpui::Div>>,
    active: bool,
    pinned: bool,
    audio_control: bool,
    suppress_tooltips: bool,
    group: SharedString,
    motion: Option<TabMotion>,
    reorder_transition: gpui_motion::Transition,
    position_offset: f32,
    reorder_key: u64,
    focus_handle: FocusHandle,
}

impl TabParts {
    /// Builds a tab's parts with its icon `icon_size` across. `position` is
    /// its place among the tabs shown, and `focus_index` its place in
    /// keyboard order; its audio and close buttons follow it.
    fn new(
        tab: TabItem,
        position: usize,
        tab_count: usize,
        focus_index: isize,
        icon_size: f32,
        palette: ThemeColors,
        axis: TabDragAxis,
    ) -> Self {
        let close_label = format!("Close {}", tab.label).into();
        let tab_label = if tab.sleeping {
            format!("{} (sleeping)", tab.label).into()
        } else {
            tab.label.clone()
        };
        let close_id = format!("{}-close", tab.id).into();
        let tab_id = tab.id.clone();
        let reorder_key = tab.reorder_key;
        let audio_control = matches!(&tab.icon, TabIcon::Audio { .. });
        let position_offset = tab.position_offset;
        let animations = tab.animations;
        let motion = tab.motion.clone();
        let reorder_transition = tab.reorder_transition;
        let dragged = tab.dragging;
        let sleeping = tab.sleeping;
        let suppress_tooltips = tab.suppress_tooltips;
        let focus_handle = tab.focus_handle.tab_index(focus_index).tab_stop(tab.active);
        let element = h_stack()
            .id(tab.id.clone())
            .group(tab.id.clone())
            .role(Role::Tab)
            .aria_label(tab_label)
            .aria_selected(tab.active)
            .aria_position_in_set(position + 1)
            .aria_size_of_set(tab_count)
            .track_focus(&focus_handle)
            .focus_ring(palette)
            .items_center()
            .text_color(rgb(if sleeping {
                palette.text_secondary
            } else if tab.active {
                palette.text_primary
            } else {
                palette.text_secondary
            }))
            .owns_clicks()
            .pressable_with_focus(focus_handle.clone())
            .on_click(tab.on_select)
            .on_key_down(tab.on_key_down)
            .on_mouse_up(MouseButton::Middle, tab.on_middle_click)
            .on_mouse_down(MouseButton::Right, tab.on_context_menu);
        let on_prepaint = tab.on_prepaint;
        let element = element.on_prepaint(move |prepaint, window, cx| {
            on_prepaint(prepaint.bounds, window, cx);
        });
        let element = element.on_mouse_down_all(move |event, phase, hitbox, window, cx| {
            (tab.on_pointer_down)(event, phase, hitbox, axis, window, cx);
        });
        let on_drop = tab.on_drop;
        let element = element
            .on_drag_over::<DraggedTab>(move |style, _, _, _| style.bg(rgba(palette.hover_surface)))
            .on_drop(move |dragged: &DraggedTab, window, cx| on_drop(dragged, window, cx));
        let element = element
            .relative()
            .when(axis == TabDragAxis::Horizontal, |element| {
                element.left(px(position_offset))
            })
            .when(axis == TabDragAxis::Vertical, |element| {
                element.top(px(position_offset))
            })
            .opacity(if dragged {
                crate::platform::theme_config::theme()
                    .opacity
                    .tab_drag_source
            } else if sleeping {
                crate::platform::theme_config::theme().opacity.sleeping_tab
            } else {
                1.0
            });

        let icon_color = if tab.active {
            palette.text_primary
        } else {
            palette.text_secondary
        };
        let icon_id: ElementId = format!("{tab_id}-icon-{:?}", tab.icon.revealed()).into();
        let icon = match tab.icon {
            TabIcon::Loading(step) => {
                loading_spinner(icon_color, icon_size, step).into_any_element()
            }
            TabIcon::Page => globe_icon(icon_color, icon_size).into_any_element(),
            TabIcon::Sleeping => sleeping_icon(icon_color, icon_size).into_any_element(),
            TabIcon::Symbol(symbol) => symbol(icon_color, icon_size),
            TabIcon::Favicon(favicon) => sized_icon(
                img(ImageSource::Render(favicon.image)).object_fit(ObjectFit::Contain),
                icon_id,
                tab.icon_appearing,
                icon_size,
                &animations,
            ),
            TabIcon::Audio { muted, .. } => audio_control_button(
                format!("{tab_id}-audio").into(),
                tab.label.clone(),
                muted,
                tab.on_toggle_audio,
                focus_index + 1,
                icon_color,
                !suppress_tooltips,
                palette,
                &animations,
            ),
            TabIcon::Logo => sized_icon(
                img(photon_logo(palette.logo_tint)).object_fit(ObjectFit::Contain),
                icon_id,
                tab.icon_appearing,
                icon_size,
                &animations,
            ),
        };
        let close = close_tab_button(
            close_id,
            close_label,
            tab.on_close,
            focus_index + 1 + if audio_control { 1 } else { 0 },
            palette,
        );
        Self {
            tab: element,
            icon,
            label: tab.label,
            close,
            active: tab.active,
            pinned: tab.pinned,
            audio_control,
            suppress_tooltips,
            group: tab_id,
            motion,
            reorder_transition,
            position_offset,
            reorder_key,
            focus_handle,
        }
    }

    /// How many keyboard stops the tab takes: itself, its close button, and
    /// its audio button when it has one.
    fn focus_stops(icon: &TabIcon) -> isize {
        if matches!(icon, TabIcon::Audio { .. }) {
            3
        } else {
            2
        }
    }
}

fn measured_tab(
    element: AnyElement,
    key: u64,
    focus_handle: FocusHandle,
    transition: gpui_motion::Transition,
    reorder: &ReorderGroup<u64>,
) -> AnyElement {
    reorder
        .item_with_focus(
            format!("browser-tab-reorder-{key}"),
            key,
            focus_handle,
            element,
        )
        .transition(transition)
        .drag_listener(false)
        .into_any_element()
}

pub(super) fn sync_reorder_order(reorder: &ReorderGroup<u64>, tabs: &[TabItem], pinned: bool) {
    if reorder.is_dragging() {
        return;
    }

    let current = reorder.order();
    if current.iter().copied().eq(tabs
        .iter()
        .filter(|tab| tab.pinned == pinned)
        .map(|tab| tab.reorder_key))
    {
        return;
    }
    drop(current);

    reorder
        .set_order(
            tabs.iter()
                .filter(|tab| tab.pinned == pinned)
                .map(|tab| tab.reorder_key)
                .collect(),
        )
        .expect("tab IDs are unique");
}

pub(super) fn reorder_tab_items(
    mut tabs: Vec<TabItem>,
    pinned_reorder: &ReorderGroup<u64>,
    today_reorder: &ReorderGroup<u64>,
) -> Vec<TabItem> {
    let reorder_pinned =
        pinned_reorder.is_dragging() && !reorder_order_matches(pinned_reorder, &tabs, true);
    let reorder_today =
        today_reorder.is_dragging() && !reorder_order_matches(today_reorder, &tabs, false);
    if !reorder_pinned && !reorder_today {
        return tabs;
    }

    if reorder_pinned {
        reorder_section(&mut tabs, pinned_reorder, true);
    }
    if reorder_today {
        reorder_section(&mut tabs, today_reorder, false);
    }
    tabs
}

fn reorder_order_matches(reorder: &ReorderGroup<u64>, tabs: &[TabItem], pinned: bool) -> bool {
    reorder.order().iter().copied().eq(tabs
        .iter()
        .filter(|tab| tab.pinned == pinned)
        .map(|tab| tab.reorder_key))
}

fn reorder_section(tabs: &mut [TabItem], reorder: &ReorderGroup<u64>, pinned: bool) {
    if !reorder.is_dragging() {
        return;
    }

    let order = reorder.order();
    let count = tabs.iter().filter(|tab| tab.pinned == pinned).count();
    if count != order.len()
        || order.iter().any(|key| {
            !tabs
                .iter()
                .any(|tab| tab.pinned == pinned && tab.reorder_key == *key)
        })
    {
        return;
    }

    // Pointer-driven order changes need no temporary allocations.
    for (target_index, key) in order.iter().enumerate() {
        let target = tabs
            .iter()
            .enumerate()
            .filter(|(_, tab)| tab.pinned == pinned)
            .nth(target_index)
            .map(|(index, _)| index)
            .expect("reorder group matches rendered tabs");
        if tabs[target].reorder_key == *key {
            continue;
        }
        let source = tabs
            .iter()
            .position(|tab| tab.pinned == pinned && tab.reorder_key == *key)
            .expect("tab key belongs to reorder group");
        tabs.swap(target, source);
    }
}

/// The temporary insertion gap shown when a drag hovers over another strip.
pub(super) fn insertion_marker(axis: TabDragAxis, size: f32, palette: ThemeColors) -> AnyElement {
    let marker = div()
        .flex_shrink_0()
        .rounded(px(metrics::control_radius()))
        .bg(rgba(palette.chosen));
    match axis {
        TabDragAxis::Horizontal => marker
            .w(px(size))
            .h(px(metrics::TAB_HEIGHT))
            .into_any_element(),
        TabDragAxis::Vertical => marker.w_full().h(px(size)).into_any_element(),
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "keeps the tab audio button's accessibility and focus inputs explicit"
)]
fn audio_control_button(
    id: SharedString,
    tab_label: SharedString,
    muted: bool,
    on_click: ClickHandler,
    focus_index: isize,
    icon_color: u32,
    show_tooltip: bool,
    palette: ThemeColors,
    animations: &super::motion::MotionConfig,
) -> AnyElement {
    let icon_id = format!("{id}-icon");
    let label = if muted {
        format!("Unmute {tab_label}")
    } else {
        format!("Mute {tab_label}")
    };
    let tooltip = super::tooltip::label_tooltip(label.clone(), palette);
    div()
        .id(id)
        .role(Role::Button)
        .aria_label(label)
        .tab_index(focus_index)
        .tab_stop(true)
        .focus_ring(palette)
        .flex()
        .flex_shrink_0()
        .relative()
        .items_center()
        .justify_center()
        .size(px(metrics::TAB_AUDIO_BUTTON_SIZE))
        // The button's hover surface outgrows the icon, but the icon sits
        // exactly where a favicon would, so the title doesn't move.
        .m(px(-metrics::TAB_AUDIO_BUTTON_OUTSET))
        .rounded(px(metrics::control_radius()))
        .text_color(rgb(icon_color))
        .surface_fade()
        .hover_style(|style| style.bg(rgba(palette.hover_surface)))
        .owns_clicks()
        .pressable()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(on_click)
        .when(show_tooltip, |button| button.tooltip(tooltip))
        .child(audio_icon(
            icon_id,
            icon_color,
            metrics::TAB_AUDIO_ICON_SIZE,
            muted,
            animations,
        ))
        .into_any_element()
}

fn close_tab_button(
    id: SharedString,
    label: SharedString,
    on_click: ClickHandler,
    focus_index: isize,
    palette: ThemeColors,
) -> gpui_motion::PressScaleElement<gpui::Stateful<gpui::Div>> {
    h_stack()
        .id(id)
        .role(Role::Button)
        .aria_label(label)
        .tab_index(focus_index)
        .focus_ring(palette)
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .size(px(metrics::TAB_CLOSE_BUTTON_SIZE))
        .rounded(px(metrics::control_radius()))
        .surface_fade()
        .hover_style(|style| style.bg(rgba(palette.hover_surface)))
        .owns_clicks()
        .pressable()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(on_click)
        .child(close_icon(palette.text_secondary, metrics::TAB_ICON_SIZE))
}
