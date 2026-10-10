//! The sidebar's vertical tab list and its rows.

use gpui::{
    AnyElement, App, Context, ElementId, FocusHandle, ImageSource, KeyDownEvent, MouseButton,
    MouseDownEvent, MouseUpEvent, ObjectFit, Render, Role, SharedString, Window, div, img,
    prelude::*, px, rgb, rgba,
};

use super::super::Favicon;
use super::super::icons::{
    add_icon, audio_icon, close_icon, globe_icon, loading_spinner, photon_logo,
};
use super::super::layout::{Elevated, Elevation, h_stack, v_stack};
use super::super::motion::{AnimateIn, Entrance};
use super::super::{metrics, theme::ThemeColors};

use super::super::ClickHandler;

/// A tab being dragged to a new position.
pub(in super::super) struct DraggedTab {
    /// The tab's position when the drag started.
    pub index: usize,
}

/// What follows the cursor while a tab is dragged.
struct TabDragPreview {
    label: SharedString,
    palette: ThemeColors,
}

impl Render for TabDragPreview {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        let palette = self.palette;
        h_stack()
            .items_center()
            .h(px(metrics::TAB_HEIGHT))
            .max_w(px(metrics::TAB_MAX_WIDTH))
            .px(px(metrics::TAB_HORIZONTAL_PADDING))
            .rounded(px(metrics::CONTROL_RADIUS))
            .border_1()
            .border_color(rgba(palette.menu_border))
            .bg(rgba(palette.menu_surface))
            .text_size(px(metrics::TAB_FONT_SIZE))
            .text_color(rgb(palette.text_primary))
            .elevated(Elevation::Medium)
            .opacity(metrics::TAB_DRAG_PREVIEW_OPACITY)
            .child(div().truncate().child(self.label.clone()))
    }
}

/// A callback for a mouse button pressed on a tab.
pub(in super::super) type TabMouseDown = Box<dyn Fn(&MouseDownEvent, &mut Window, &mut App)>;
/// A callback for a mouse button released on a tab.
pub(in super::super) type TabMouseUp = Box<dyn Fn(&MouseUpEvent, &mut Window, &mut App)>;
/// A callback for a tab dropped onto another.
pub(in super::super) type TabDrop = Box<dyn Fn(&DraggedTab, &mut Window, &mut App)>;

/// What a tab shows before its title.
pub(in super::super) enum TabIcon {
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
pub(in super::super) enum RevealedIcon {
    Logo,
    Favicon(u64),
}

impl TabIcon {
    /// The icon the appear animation reveals. The spinner and the generic
    /// page icon are placeholders, so they neither animate nor replace the
    /// last revealed icon: a page's icon that returns after them stays put.
    pub(in super::super) fn revealed(&self) -> Option<RevealedIcon> {
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
pub(in super::super) const ICON_ENTRANCE: Entrance = Entrance::pop(metrics::TAB_FAVICON_SIZE);

/// Sizes an icon image, growing it in from a smaller size while
/// `appearing`, inside a fixed slot so the tab title does not move.
fn sized_icon<E>(icon: E, id: ElementId, appearing: bool, size: f32) -> AnyElement
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
        .child(icon.animate_in(id, ICON_ENTRANCE))
        .into_any_element()
}

pub(in super::super) struct TabItem {
    pub id: String,
    pub label: String,
    pub icon: TabIcon,
    /// Whether the icon is still growing in after it first appeared.
    pub icon_appearing: bool,
    pub active: bool,
    pub focus_handle: FocusHandle,
    pub on_select: ClickHandler,
    pub on_key_down: Box<dyn Fn(&KeyDownEvent, &mut Window, &mut App)>,
    pub on_close: ClickHandler,
    pub on_toggle_audio: ClickHandler,
    /// Middle-click closes the tab.
    pub on_middle_click: TabMouseUp,
    /// Right-click opens the tab's menu.
    pub on_context_menu: TabMouseDown,
    /// A dragged tab dropped onto this one moves to its place.
    pub on_drop: TabDrop,
}

/// The tabs, one row each, under a row that opens a new tab.
pub(in super::super) fn tab_list(
    tabs: Vec<TabItem>,
    on_new_tab: ClickHandler,
    palette: ThemeColors,
) -> impl IntoElement {
    let tab_count = tabs.len();
    let mut focus_index = 1;
    let mut rows = Vec::with_capacity(tab_count);
    for (index, tab) in tabs.into_iter().enumerate() {
        let audio_control = matches!(&tab.icon, TabIcon::Audio { .. });
        rows.push(tab_row(tab, index, tab_count, focus_index, palette));
        focus_index += if audio_control { 3 } else { 2 };
    }
    v_stack()
        .w_full()
        .gap(px(metrics::SIDEBAR_ITEM_GAP))
        .tab_group()
        .child(new_tab_row(on_new_tab, palette))
        .child(
            v_stack()
                .id("browser-tab-list")
                .role(Role::TabList)
                .aria_label("Browser tabs")
                .w_full()
                .gap(px(metrics::SIDEBAR_ITEM_GAP))
                .children(rows),
        )
}

fn tab_row(
    tab: TabItem,
    index: usize,
    tab_count: usize,
    focus_index: isize,
    palette: ThemeColors,
) -> impl IntoElement {
    let close_label = format!("Close {}", tab.label);
    let close_id = format!("{}-close", tab.id);
    let tab_id = tab.id.clone();
    let audio_control = matches!(&tab.icon, TabIcon::Audio { .. });
    let focus_handle = tab.focus_handle.tab_index(focus_index).tab_stop(tab.active);
    let mut row = h_stack()
        .id(tab.id.clone())
        .group(tab.id)
        .role(Role::Tab)
        .aria_label(tab.label.clone())
        .aria_selected(tab.active)
        .aria_position_in_set(index + 1)
        .aria_size_of_set(tab_count)
        .track_focus(&focus_handle)
        .focus_visible(|style| style.bg(rgba(palette.hover_surface)))
        .w_full()
        .items_center()
        .gap(px(metrics::OMNIBOX_GAP))
        .h(px(metrics::SIDEBAR_TAB_HEIGHT))
        .pl(px(metrics::SIDEBAR_TAB_PADDING))
        .pr(px(metrics::SIDEBAR_ITEM_GAP * 2.0))
        .rounded(px(metrics::SIDEBAR_ITEM_RADIUS))
        .text_size(px(metrics::SIDEBAR_FONT_SIZE))
        .text_color(rgb(if tab.active {
            palette.text_primary
        } else {
            palette.text_secondary
        }))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(tab.on_select)
        .on_key_down(tab.on_key_down)
        .on_mouse_up(MouseButton::Middle, tab.on_middle_click)
        .on_mouse_down(MouseButton::Right, tab.on_context_menu)
        .on_drag(DraggedTab { index }, {
            let label = SharedString::from(tab.label.clone());
            move |_, _, _, cx| {
                let label = label.clone();
                cx.new(|_| TabDragPreview { label, palette })
            }
        })
        .drag_over::<DraggedTab>(move |style, _, _, _| style.bg(rgba(palette.hover_surface)))
        .on_drop(tab.on_drop);

    row = if tab.active {
        row.bg(rgba(palette.surface))
    } else {
        row.hover(|style| style.bg(rgba(palette.hover_surface)))
    };

    let icon_color = if tab.active {
        palette.text_primary
    } else {
        palette.text_secondary
    };
    let icon_size = metrics::TAB_FAVICON_SIZE;
    let icon_id: ElementId = format!("{tab_id}-icon-{:?}", tab.icon.revealed()).into();
    let icon = match tab.icon {
        TabIcon::Loading(step) => loading_spinner(icon_color, icon_size, step).into_any_element(),
        TabIcon::Page => globe_icon(icon_color, icon_size).into_any_element(),
        TabIcon::Symbol(symbol) => symbol(icon_color, icon_size),
        TabIcon::Favicon(favicon) => sized_icon(
            img(ImageSource::Render(favicon.image)).object_fit(ObjectFit::Contain),
            icon_id,
            tab.icon_appearing,
            icon_size,
        ),
        TabIcon::Audio { muted, .. } => audio_control_button(
            format!("{tab_id}-audio"),
            tab.label.clone(),
            muted,
            tab.on_toggle_audio,
            focus_index + 1,
            icon_color,
            palette,
        ),
        TabIcon::Logo => sized_icon(
            img(photon_logo()).object_fit(ObjectFit::Contain),
            icon_id,
            tab.icon_appearing,
            icon_size,
        ),
    };

    // The close button shows on the active tab and on the row under the
    // pointer. Hidden, it still sits under the pointer only when its row is
    // hovered, so it is never clicked unseen.
    let close = close_tab_button(
        close_id,
        close_label,
        tab.on_close,
        focus_index + 1 + if audio_control { 1 } else { 0 },
        palette,
    );
    let close = if tab.active {
        close
    } else {
        close
            .opacity(0.0)
            .group_hover(tab_id, |style| style.opacity(1.0))
    };

    row.child(icon)
        .child(div().flex_1().min_w_0().truncate().child(tab.label))
        .child(close)
}

fn audio_control_button(
    id: String,
    tab_label: String,
    muted: bool,
    on_click: ClickHandler,
    focus_index: isize,
    icon_color: u32,
    palette: ThemeColors,
) -> AnyElement {
    let label = if muted {
        format!("Unmute {tab_label}")
    } else {
        format!("Mute {tab_label}")
    };
    div()
        .id(id)
        .role(Role::Button)
        .aria_label(label)
        .tab_index(focus_index)
        .focus_visible(|style| style.border_1().border_color(rgb(palette.chosen)))
        .flex_shrink_0()
        .relative()
        .items_center()
        .justify_center()
        .size(px(metrics::TAB_AUDIO_BUTTON_SIZE))
        .rounded(px(metrics::CONTROL_RADIUS))
        .text_color(rgb(icon_color))
        .hover(|style| style.bg(rgba(palette.hover_surface)))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(on_click)
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
        .focus_visible(|style| style.border_1().border_color(rgb(palette.chosen)))
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .size(px(metrics::TAB_CLOSE_BUTTON_SIZE))
        .rounded(px(metrics::CONTROL_RADIUS))
        .hover(|style| style.bg(rgba(palette.hover_surface)))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(on_click)
        .child(close_icon(palette.text_secondary, metrics::TAB_ICON_SIZE))
}

/// "+ New Tab", above the tabs.
fn new_tab_row(on_click: ClickHandler, palette: ThemeColors) -> impl IntoElement {
    h_stack()
        .id("browser-new-tab")
        .role(Role::Button)
        .aria_label("New tab")
        .tab_index(0)
        .focus_visible(|style| style.border_1().border_color(rgb(palette.chosen)))
        .w_full()
        .items_center()
        .gap(px(metrics::OMNIBOX_GAP))
        .h(px(metrics::SIDEBAR_TAB_HEIGHT))
        .px(px(metrics::SIDEBAR_TAB_PADDING))
        .rounded(px(metrics::SIDEBAR_ITEM_RADIUS))
        .text_size(px(metrics::SIDEBAR_FONT_SIZE))
        .text_color(rgb(palette.text_secondary))
        .hover(|style| style.bg(rgba(palette.hover_surface)))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(on_click)
        .child(add_icon(palette.text_secondary, metrics::TAB_FAVICON_SIZE))
        .child("New Tab")
}
