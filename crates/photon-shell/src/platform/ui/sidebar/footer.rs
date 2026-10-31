//! The sidebar's footer: downloads at the left, the spaces in the middle,
//! and the browser menu at the right. Collapsed to icons, the spaces stack
//! above downloads and the browser menu.

use gpui::{App, MouseButton, Orientation, Role, Window, div, prelude::*, px, rgb, rgba};

use super::super::animated_icons::{animated_icon, plus};
use super::super::button::{menu_button, toolbar_button};
use super::super::controls::{FocusRing, RovingAxis, RovingFocus};
use super::super::controls::{OwnsClicks, Pressable};
use super::super::download_button::{DownloadsButton, downloads_button};
use super::super::layout::{h_stack, v_stack};
use super::super::motion::SurfaceFade;
use super::super::tooltip::label_tooltip;
use super::super::{ClickHandler, MouseDownHandler, metrics, theme::ThemeColors};

pub(in super::super) type PrepaintHandler =
    Box<dyn Fn(&gpui::InteractivityPrepaint, &mut Window, &mut App)>;

/// Holds the downloads button and reports where it is drawn.
fn downloads_anchor(on_drawn: PrepaintHandler) -> gpui::Stateful<gpui::Div> {
    div().id("sidebar-downloads-anchor").on_prepaint(on_drawn)
}

pub(in super::super) struct FooterActions {
    /// Opens the downloads popover.
    pub(in super::super) downloads: ClickHandler,
    pub(in super::super) downloads_state: DownloadsButton,
    /// Told where the downloads button is drawn, which its popover points at.
    pub(in super::super) downloads_drawn: PrepaintHandler,
    pub(in super::super) menu: ClickHandler,
    /// Adds a space; `None` in a private window, which cannot.
    pub(in super::super) new_space: Option<ClickHandler>,
}

/// A space, shown as a dot of its colour.
pub(in super::super) struct SpaceDot {
    /// The space's stable id, which keys its focus and press state.
    pub(in super::super) id: photon_core::SpaceId,
    pub(in super::super) name: String,
    /// The space's colour, as an RGB token.
    pub(in super::super) color: u32,
    pub(in super::super) active: bool,
    pub(in super::super) on_select: std::rc::Rc<ClickHandler>,
    /// Right-click opens the space's menu.
    pub(in super::super) on_context_menu: MouseDownHandler,
}

pub(in super::super) fn footer(
    spaces: Vec<SpaceDot>,
    actions: FooterActions,
    collapsed: bool,
    palette: ThemeColors,
    window: &mut Window,
    cx: &mut App,
) -> gpui::AnyElement {
    let focus = (spaces.len() > 1).then(|| {
        let selected = spaces.iter().position(|space| space.active).unwrap_or(0);
        RovingFocus::keyed(
            "sidebar-spaces",
            selected,
            spaces.iter().map(|space| space.id).collect(),
            spaces.iter().map(|space| space.on_select.clone()).collect(),
            window,
            cx,
        )
    });
    let menu = menu_button("sidebar-menu", 2, palette, actions.menu, window, cx)
        .tooltip(label_tooltip("Open browser menu", palette));
    let new_space = actions.new_space.map(|on_click| {
        toolbar_button(
            "sidebar-new-space",
            "New space",
            1,
            true,
            animated_icon(
                "sidebar-new-space",
                plus(),
                palette.text_secondary,
                metrics::TOOLBAR_ICON_SIZE,
                window,
                cx,
            )
            .size_full(),
            palette,
            on_click,
        )
    });
    if collapsed {
        let spacing = metrics::SidebarSpacing::new(collapsed);
        let menu = super::collapsed_tile(menu);
        let downloads = downloads_anchor(actions.downloads_drawn).child(super::collapsed_tile(
            downloads_button(
                "sidebar-downloads",
                0,
                actions.downloads_state,
                palette,
                window,
                cx,
                actions.downloads,
            )
            .tooltip(label_tooltip("Downloads", palette)),
        ));
        let new_space = new_space.map(|button| {
            super::collapsed_tile(button.tooltip(label_tooltip("New space", palette)))
        });
        return v_stack()
            .w_full()
            .flex_shrink_0()
            .items_center()
            .pt(px(spacing.padding))
            .pb(px(metrics::PAGE_INSET))
            .gap(px(spacing.item_gap))
            .child(space_switcher(
                v_stack(),
                spaces,
                palette,
                focus,
                RovingAxis::Vertical,
            ))
            .children(new_space)
            .child(downloads)
            .child(menu)
            .into_any_element();
    }
    h_stack()
        .w_full()
        .h(px(metrics::SIDEBAR_FOOTER_HEIGHT))
        .flex_shrink_0()
        .items_center()
        .justify_between()
        .px(px(metrics::SIDEBAR_PADDING))
        .child(
            downloads_anchor(actions.downloads_drawn).child(downloads_button(
                "sidebar-downloads",
                0,
                actions.downloads_state,
                palette,
                window,
                cx,
                actions.downloads,
            )),
        )
        .child(
            h_stack()
                .items_center()
                .gap(px(metrics::SPACE_DOT_GAP))
                .child(space_switcher(
                    h_stack(),
                    spaces,
                    palette,
                    focus,
                    RovingAxis::Horizontal,
                ))
                .children(new_space),
        )
        .child(menu)
        .into_any_element()
}

/// The spaces as dots in `stack`, a row or a column. One space needs no
/// switcher, so it shows none.
fn space_switcher(
    stack: gpui::Div,
    spaces: Vec<SpaceDot>,
    palette: ThemeColors,
    focus: Option<RovingFocus>,
    axis: RovingAxis,
) -> gpui::Stateful<gpui::Div> {
    let has_multiple_spaces = spaces.len() > 1;
    let dots = has_multiple_spaces.then(|| {
        let focus = focus.expect("space tabs have roving keyboard focus");
        spaces
            .into_iter()
            .enumerate()
            .map(move |(index, space)| space_dot(index, space, palette, focus.clone(), axis))
    });
    stack
        .id("sidebar-spaces")
        .when(has_multiple_spaces, |stack| {
            stack
                .role(Role::TabList)
                .aria_label("Spaces")
                .tab_group()
                .when(matches!(axis, RovingAxis::Vertical), |stack| {
                    stack.aria_orientation(Orientation::Vertical)
                })
        })
        .items_center()
        .gap(px(metrics::SPACE_DOT_GAP))
        .children(dots.into_iter().flatten())
}

/// A dot of the space's colour, larger and ringed while it is shown.
fn space_dot(
    index: usize,
    space: SpaceDot,
    palette: ThemeColors,
    focus: RovingFocus,
    axis: RovingAxis,
) -> impl IntoElement {
    let tooltip = label_tooltip(space.name.clone(), palette);
    let size = if space.active {
        metrics::SPACE_DOT_ACTIVE_SIZE
    } else {
        metrics::SPACE_DOT_SIZE
    };
    h_stack()
        .id(gpui::ElementId::Name(
            format!("sidebar-space-{}", space.id).into(),
        ))
        .role(Role::Tab)
        .aria_label(space.name)
        .aria_selected(space.active)
        .track_focus(&focus.handle(index))
        .tab_index(index as isize)
        .tab_stop(focus.selected(index))
        .focus_ring(palette)
        .size(px(metrics::SPACE_DOT_BUTTON_SIZE))
        .items_center()
        .justify_center()
        .rounded_full()
        .surface_fade()
        .hover_style(|style| style.bg(rgba(palette.hover_surface)))
        .owns_clicks()
        .pressable()
        .on_mouse_down(MouseButton::Right, space.on_context_menu)
        .on_click(focus.on_click(index))
        .on_key_down(focus.on_key_down(index, axis))
        .tooltip(tooltip)
        .child(
            div()
                .size(px(size))
                .rounded_full()
                .bg(rgb(space.color))
                .when(!space.active, |dot| {
                    dot.opacity(
                        crate::platform::theme_config::theme()
                            .opacity
                            .space_dot_idle,
                    )
                }),
        )
}
