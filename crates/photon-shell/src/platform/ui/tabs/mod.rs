//! Browser tabs, shared by the horizontal tab strip and the sidebar's
//! vertical list: a tab's model, its interactions, icon and close button.
//!
//! - `strip`: the horizontal tab strip in the titlebar.
//! - `list`: the vertical tab list in the sidebar.

mod list;
mod strip;

use gpui::{
    AnimationExt, AnyElement, AnyWindowHandle, App, Bounds, Context, DispatchPhase, ElementId,
    FocusHandle, Hitbox, ImageSource, MouseButton, MouseDownEvent, MouseMoveEvent, ObjectFit,
    Render, Role, SpringAnimation, SpringConfig, Stateful, Window, div, img, prelude::*, px, rgb,
    rgba,
};

use super::Favicon;
use super::controls::FocusRing;
use super::controls::{OwnsClicks, Pressable};
use super::icons::{
    audio_icon, close_icon, globe_icon, loading_spinner, photon_logo, sleeping_icon,
};
use super::layout::{h_stack, v_stack};
use super::motion::{AnimateIn, Entrance};
use super::{
    ClickHandler, KeyDownHandler, MouseDownHandler, MouseUpHandler, metrics, theme::ThemeColors,
};
pub(super) use list::tab_list;
use std::{rc::Rc, time::Duration};
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
pub(super) const ICON_ENTRANCE: Entrance = Entrance::pop(metrics::TAB_FAVICON_SIZE);

/// A closing tab settles out of its slot with the same spring used by its entry.
const TAB_MOTION_SPRING: SpringConfig = SpringConfig::new(1_800.0, 84.85, 1.0);
const TAB_MOTION_EPSILON: f32 = 0.001;
pub(super) const TAB_CLOSE_DURATION: Duration = Duration::from_millis(260);

#[derive(Clone)]
pub(super) struct TabMotion {
    pub id: ElementId,
    pub from: f32,
    pub target: f32,
    pub closing: bool,
    pub extent: f32,
    pub trailing_gap: f32,
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
    let element = if motion.closing {
        match axis {
            TabDragAxis::Horizontal => element
                .absolute()
                .flex_none()
                .min_w_0()
                .w(px(motion.extent))
                .top(px(0.0))
                .h(px(metrics::TAB_HEIGHT)),
            TabDragAxis::Vertical => element
                .absolute()
                .flex_none()
                .min_h_0()
                .left(px(0.0))
                .w_full()
                .h(px(motion.extent)),
        }
    } else {
        element
    };
    let element = element.with_spring(
        motion.id,
        SpringAnimation::new(TAB_MOTION_SPRING)
            .with_epsilon(TAB_MOTION_EPSILON)
            .to(motion.target)
            .from(motion.from),
        move |element, value| {
            let progress = value.clamp(0.0, 1.0);
            let leaving = 1.0 - progress;
            let offset = match axis {
                TabDragAxis::Horizontal => {
                    -metrics::TAB_ENTRY_OFFSET * leaving * motion.offset_scale
                }
                TabDragAxis::Vertical => metrics::TAB_ENTRY_OFFSET * leaving * motion.offset_scale,
            };
            element
                .opacity(motion.opacity * progress)
                .when(axis == TabDragAxis::Horizontal, |element| {
                    element.left(px(position_offset + offset))
                })
                .when(axis == TabDragAxis::Vertical, |element| {
                    element.top(px(position_offset + offset))
                })
        },
    );
    if !motion.closing {
        return element.into_any_element();
    }

    match axis {
        TabDragAxis::Horizontal => h_stack()
            .relative()
            .flex_none()
            .min_w_0()
            .w(px(0.0))
            .h(px(metrics::TAB_HEIGHT))
            .mr(px(-motion.trailing_gap))
            .child(element)
            .into_any_element(),
        TabDragAxis::Vertical => v_stack()
            .relative()
            .flex_none()
            .min_h_0()
            .w_full()
            .h(px(0.0))
            .mb(px(-motion.trailing_gap))
            .child(element)
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
    /// The tab's entrance or in-place closing spring.
    pub motion: Option<TabMotion>,
    /// The tab is leaving and should no longer respond to input.
    pub closing: bool,
    /// The tab's place among the window's tabs, which a list may not show
    /// all of.
    pub index: usize,
    pub label: String,
    pub icon: TabIcon,
    /// Whether the icon is still growing in after it first appeared.
    pub icon_appearing: bool,
    pub animations: photon_core::AnimationSettings,
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
    tab: Stateful<gpui::Div>,
    icon: AnyElement,
    label: String,
    close: Stateful<gpui::Div>,
    active: bool,
    pinned: bool,
    suppress_tooltips: bool,
    group: String,
    motion: Option<TabMotion>,
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
        let tab_label = if tab.sleeping {
            format!("{} (sleeping)", tab.label)
        } else {
            tab.label.clone()
        };
        let close_id = format!("{}-close", tab.id);
        let tab_id = tab.id.clone();
        let audio_control = matches!(&tab.icon, TabIcon::Audio { .. });
        let position_offset = tab.position_offset;
        let animations = tab.animations;
        let motion = tab.motion.clone();
        let dragged = tab.dragging;
        let sleeping = tab.sleeping;
        let suppress_tooltips = tab.suppress_tooltips;
        let focus_handle = tab
            .focus_handle
            .tab_index(focus_index)
            .tab_stop(tab.active && !tab.closing);
        let element = h_stack()
            .id(tab.id.clone())
            .group(tab.id.clone())
            .role(Role::Tab)
            .aria_label(tab_label)
            .aria_selected(tab.active && !tab.closing)
            .aria_position_in_set(position + 1)
            .aria_size_of_set(tab_count)
            .when(!tab.closing, |element| element.track_focus(&focus_handle))
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
            } else if sleeping {
                metrics::SLEEPING_TAB_OPACITY
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
                format!("{tab_id}-audio"),
                tab.label.clone(),
                muted,
                tab.on_toggle_audio,
                focus_index + 1,
                icon_color,
                !suppress_tooltips,
                !tab.closing,
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
        let close = if tab.closing {
            close.tab_stop(false)
        } else {
            close
        };
        Self {
            tab: element,
            icon,
            label: tab.label,
            close,
            active: tab.active,
            pinned: tab.pinned,
            suppress_tooltips,
            group: tab_id,
            motion,
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
    tab_stop: bool,
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
        .tab_stop(tab_stop)
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
