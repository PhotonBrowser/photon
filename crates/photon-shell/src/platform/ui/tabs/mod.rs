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
    Stateful, Window, div, img, prelude::*, px, rgb, rgba,
};

use super::Favicon;
use super::controls::FocusRing;
use super::controls::{OwnsClicks, Pressable};
use super::icons::{audio_icon, close_icon, globe_icon, loading_spinner, photon_logo};
use super::layout::h_stack;
use super::motion::{AnimateIn, Edge, Entrance, Speed};
use super::{
    ClickHandler, KeyDownHandler, MouseDownHandler, MouseUpHandler, metrics, theme::ThemeColors,
};
pub(super) use list::tab_list;
use std::{
    rc::Rc,
    sync::Arc,
    time::{Duration, Instant},
};
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

/// A stable drag payload shared by both tab layouts and native drop targets.
#[derive(Clone)]
pub(super) struct DraggedTab {
    pub source: AnyWindowHandle,
    pub id: u64,
    pub size: f32,
    pub pinned: bool,
}

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
            Self::Loading(_) | Self::Page | Self::Symbol(_) | Self::Audio { favicon: None, .. } => {
                None
            }
        }
    }
}

/// How a tab icon appears.
pub(super) const ICON_ENTRANCE: Entrance = Entrance::pop(metrics::TAB_FAVICON_SIZE);

/// A new tab settles into its slot as it fades and slides into view.
pub(super) const TAB_ENTRY_MOTION: Entrance = Entrance::new().fade_from(0.0).speed(Speed::Standard);

pub(super) fn animate_tab_entry<E>(
    element: E,
    animation: Option<(ElementId, photon_core::AnimationSettings)>,
    axis: TabDragAxis,
    position_offset: f32,
) -> AnyElement
where
    E: Styled + IntoElement + 'static,
{
    let Some((id, settings)) = animation else {
        return element.into_any_element();
    };
    let entrance = match axis {
        TabDragAxis::Horizontal => {
            TAB_ENTRY_MOTION.slide_from(Edge::Left, metrics::TAB_ENTRY_OFFSET)
        }
        TabDragAxis::Vertical => TAB_ENTRY_MOTION.slide_down(metrics::TAB_ENTRY_OFFSET),
    };
    let base_offset = match axis {
        TabDragAxis::Horizontal => (position_offset, 0.0),
        TabDragAxis::Vertical => (0.0, position_offset),
    };
    element
        .animate_in_with_offset(id, entrance, &settings, base_offset)
        .into_any_element()
}

/// A tab's last visible appearance, retained briefly after it closes.
pub(super) struct TabExit {
    pub id: u64,
    pub label: String,
    pub icon: TabIcon,
    pub active: bool,
    pub pinned: bool,
    pub axis: TabDragAxis,
    pub collapsed: bool,
    pub bounds: Bounds<gpui::Pixels>,
    pub started: Instant,
    pub duration: Duration,
    /// Keep one fully faded frame mounted before removing this visual.
    pub finished_frame: bool,
    /// The tab view's favicon image must outlive its exit visual.
    pub favicon_image_to_drop: Option<Arc<gpui::RenderImage>>,
}

impl TabExit {
    pub fn shows_favicon(&self) -> bool {
        matches!(
            &self.icon,
            TabIcon::Favicon(_)
                | TabIcon::Audio {
                    favicon: Some(_),
                    ..
                }
        )
    }
}

/// Draws a just-closed tab over its last bounds while it fades and slides away.
pub(super) fn tab_exit_element(
    exit: &TabExit,
    palette: ThemeColors,
    progress: f32,
    request_next_frame: bool,
) -> AnyElement {
    let eased = progress.clamp(0.0, 1.0).powi(3);
    let opacity = 1.0 - eased;
    let offset = -metrics::TAB_ENTRY_OFFSET * eased;
    let color = if exit.active {
        palette.text_primary
    } else {
        palette.text_secondary
    };
    let icon_size = if exit.axis == TabDragAxis::Vertical {
        metrics::SidebarSpacing::new(exit.collapsed).icon_size
    } else {
        metrics::TAB_FAVICON_SIZE
    };
    let icon = tab_exit_icon(&exit.icon, color, icon_size, palette);
    let close = h_stack()
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .size(px(metrics::TAB_CLOSE_BUTTON_SIZE))
        .child(close_icon(palette.text_secondary, metrics::TAB_ICON_SIZE));
    let content = match (exit.axis, exit.collapsed, exit.pinned) {
        (TabDragAxis::Horizontal, _, true) => div()
            .size_full()
            .items_center()
            .justify_center()
            .child(icon),
        (TabDragAxis::Vertical, true, _) => div()
            .size_full()
            .items_center()
            .justify_center()
            .child(icon),
        (TabDragAxis::Horizontal, _, false) => h_stack()
            .size_full()
            .items_center()
            .gap(px(metrics::TAB_CLOSE_GAP))
            .px(px(metrics::TAB_HORIZONTAL_PADDING))
            .text_size(px(metrics::TAB_FONT_SIZE))
            .child(icon)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .child(exit.label.clone()),
            )
            .child(close),
        (TabDragAxis::Vertical, false, _) => h_stack()
            .size_full()
            .items_center()
            .gap(px(metrics::SIDEBAR_TAB_ICON_GAP))
            .pl(px(metrics::SIDEBAR_TAB_PADDING))
            .pr(px(metrics::SIDEBAR_PADDING))
            .text_size(px(metrics::SIDEBAR_FONT_SIZE))
            .child(icon)
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .truncate()
                    .child(exit.label.clone()),
            )
            .child(close),
    };
    let origin = exit.bounds.origin;
    let element = div()
        .id(format!("browser-tab-exit-{}", exit.id))
        .absolute()
        .left(px(f32::from(origin.x)
            + if exit.axis == TabDragAxis::Horizontal {
                offset
            } else {
                0.0
            }))
        .top(px(f32::from(origin.y)
            + if exit.axis == TabDragAxis::Vertical {
                offset
            } else {
                0.0
            }))
        .w(exit.bounds.size.width)
        .h(exit.bounds.size.height)
        .rounded(px(if exit.axis == TabDragAxis::Horizontal {
            metrics::CONTROL_RADIUS
        } else {
            metrics::SIDEBAR_ITEM_RADIUS
        }))
        .text_color(rgb(color))
        .when(exit.active, |element| {
            element.bg(rgba(palette.selected_surface))
        })
        .opacity(opacity)
        .child(content)
        .on_prepaint(move |_, window, _| {
            if request_next_frame {
                window.request_animation_frame();
            }
        });
    element.into_any_element()
}

fn tab_exit_icon(icon: &TabIcon, color: u32, size: f32, palette: ThemeColors) -> AnyElement {
    match icon {
        TabIcon::Loading(step) => loading_spinner(color, size, *step).into_any_element(),
        TabIcon::Favicon(favicon) => img(ImageSource::Render(favicon.image.clone()))
            .object_fit(ObjectFit::Contain)
            .size(px(size))
            .flex_shrink_0()
            .into_any_element(),
        TabIcon::Audio { muted, .. } => div()
            .size(px(metrics::TAB_AUDIO_BUTTON_SIZE))
            .flex_shrink_0()
            .items_center()
            .justify_center()
            .child(audio_icon(color, metrics::TAB_AUDIO_ICON_SIZE, *muted))
            .into_any_element(),
        TabIcon::Page => globe_icon(color, size).into_any_element(),
        TabIcon::Symbol(symbol) => symbol(color, size),
        TabIcon::Logo => img(photon_logo(palette.logo_tint))
            .object_fit(ObjectFit::Contain)
            .size(px(size))
            .flex_shrink_0()
            .into_any_element(),
    }
}

/// Sizes an icon image, growing it in from a smaller size while
/// `appearing`, inside a fixed slot so the tab title does not move.
fn sized_icon<E>(
    icon: E,
    id: ElementId,
    appearing: bool,
    size: f32,
    animations: &photon_core::AnimationSettings,
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
    pub id: String,
    /// Whether this tab is still within its first appearance animation.
    pub entry_appearing: bool,
    /// The tab's place among the window's tabs, which a list may not show
    /// all of.
    pub index: usize,
    pub label: String,
    pub icon: TabIcon,
    /// Whether the icon is still growing in after it first appeared.
    pub icon_appearing: bool,
    pub animations: photon_core::AnimationSettings,
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
    tab: Stateful<gpui::Div>,
    icon: AnyElement,
    label: String,
    close: Stateful<gpui::Div>,
    active: bool,
    pinned: bool,
    suppress_tooltips: bool,
    group: String,
    entry_animation: Option<(ElementId, photon_core::AnimationSettings)>,
    position_offset: f32,
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
        let close_label = format!("Close {}", tab.label);
        let close_id = format!("{}-close", tab.id);
        let tab_id = tab.id.clone();
        let audio_control = matches!(&tab.icon, TabIcon::Audio { .. });
        let position_offset = tab.position_offset;
        let animations = tab.animations;
        let entry_animation = tab
            .entry_appearing
            .then(|| (format!("{tab_id}-entry").into(), animations));
        let dragged = tab.dragging;
        let suppress_tooltips = tab.suppress_tooltips;
        let focus_handle = tab.focus_handle.tab_index(focus_index).tab_stop(tab.active);
        let element = h_stack()
            .id(tab.id.clone())
            .group(tab.id.clone())
            .role(Role::Tab)
            .aria_label(tab.label.clone())
            .aria_selected(tab.active)
            .aria_position_in_set(position + 1)
            .aria_size_of_set(tab_count)
            .track_focus(&focus_handle)
            .focus_ring(palette)
            .items_center()
            .text_color(rgb(if tab.active {
                palette.text_primary
            } else {
                palette.text_secondary
            }))
            .owns_clicks()
            .pressable()
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
            .drag_over::<DraggedTab>(move |style, _, _, _| style.bg(rgba(palette.hover_surface)))
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
                metrics::TAB_DRAG_SOURCE_OPACITY
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
            TabIcon::Symbol(symbol) => symbol(icon_color, icon_size),
            TabIcon::Favicon(favicon) => sized_icon(
                img(ImageSource::Render(favicon.image)).object_fit(ObjectFit::Contain),
                icon_id,
                tab.icon_appearing,
                icon_size,
                &animations,
            ),
            TabIcon::Audio { muted, .. } => audio_control_button(
                format!("{tab_id}-audio"),
                tab.label.clone(),
                muted,
                tab.on_toggle_audio,
                focus_index + 1,
                icon_color,
                !suppress_tooltips,
                palette,
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
            suppress_tooltips,
            group: tab_id,
            entry_animation,
            position_offset,
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

/// The temporary insertion gap shown when a drag hovers over another strip.
pub(super) fn insertion_marker(axis: TabDragAxis, size: f32, palette: ThemeColors) -> AnyElement {
    let marker = div()
        .flex_shrink_0()
        .rounded(px(metrics::CONTROL_RADIUS))
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
    id: String,
    tab_label: String,
    muted: bool,
    on_click: ClickHandler,
    focus_index: isize,
    icon_color: u32,
    show_tooltip: bool,
    palette: ThemeColors,
) -> AnyElement {
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
        .focus_ring(palette)
        .flex_shrink_0()
        .relative()
        .items_center()
        .justify_center()
        .size(px(metrics::TAB_AUDIO_BUTTON_SIZE))
        .rounded(px(metrics::CONTROL_RADIUS))
        .text_color(rgb(icon_color))
        .hover(|style| style.bg(rgba(palette.hover_surface)))
        .owns_clicks()
        .pressable()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(on_click)
        .when(show_tooltip, |button| button.tooltip(tooltip))
        .child(audio_icon(icon_color, metrics::TAB_AUDIO_ICON_SIZE, muted))
        .into_any_element()
}

fn close_tab_button(
    id: String,
    label: String,
    on_click: ClickHandler,
    focus_index: isize,
    palette: ThemeColors,
) -> gpui::Stateful<gpui::Div> {
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
        .rounded(px(metrics::CONTROL_RADIUS))
        .hover(|style| style.bg(rgba(palette.hover_surface)))
        .owns_clicks()
        .pressable()
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(on_click)
        .child(close_icon(palette.text_secondary, metrics::TAB_ICON_SIZE))
}
