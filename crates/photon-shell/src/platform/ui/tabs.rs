//! Browser tab strip and its controls.

use gpui::{
    AnyElement, App, Context, ElementId, FocusHandle, ImageSource, KeyDownEvent, MouseButton,
    MouseDownEvent, MouseUpEvent, ObjectFit, Render, Role, SharedString, Window, div, img,
    prelude::*, px, rgb, rgba,
};

use super::Favicon;
use super::icons::{add_icon, audio_icon, close_icon, globe_icon, loading_spinner, photon_logo};
use super::layout::h_stack;
use super::layout::{Elevated, Elevation};
use super::motion::{AnimateIn, Entrance};
use super::{metrics, theme::ThemeColors};

use super::ClickHandler;

/// A tab being dragged to a new position.
pub(super) struct DraggedTab {
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
pub(super) type TabMouseDown = Box<dyn Fn(&MouseDownEvent, &mut Window, &mut App)>;
/// A callback for a mouse button released on a tab.
pub(super) type TabMouseUp = Box<dyn Fn(&MouseUpEvent, &mut Window, &mut App)>;
/// A callback for a tab dropped onto another.
pub(super) type TabDrop = Box<dyn Fn(&DraggedTab, &mut Window, &mut App)>;

/// What a tab shows before its title.
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

pub(super) struct TabItem {
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

pub(super) fn tab_strip(
    tabs: Vec<TabItem>,
    on_new_tab: ClickHandler,
    palette: ThemeColors,
) -> impl IntoElement {
    let tab_count = tabs.len();
    let mut tab_list = h_stack()
        .id("browser-tab-list")
        .role(Role::TabList)
        .aria_label("Browser tabs")
        .items_center()
        .gap(px(metrics::TAB_STRIP_GAP))
        .flex_initial()
        .min_w_0()
        .overflow_x_scroll();

    let mut focus_index = 0;
    for (index, tab) in tabs.into_iter().enumerate() {
        let audio_control = matches!(&tab.icon, TabIcon::Audio { .. });
        tab_list = tab_list.child(browser_tab(tab, index, tab_count, focus_index, palette));
        focus_index += if audio_control { 3 } else { 2 };
    }

    h_stack()
        .w_full()
        .items_center()
        .gap(px(metrics::TAB_STRIP_GAP))
        .h(px(metrics::TITLEBAR_HEIGHT))
        .px(px(metrics::TAB_STRIP_INSET))
        .tab_group()
        .child(tab_list)
        .child(new_tab_button(on_new_tab, focus_index, palette))
}

fn browser_tab(
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
    let mut control = h_stack()
        .id(tab.id)
        .role(Role::Tab)
        .aria_label(tab.label.clone())
        .aria_selected(tab.active)
        .aria_position_in_set(index + 1)
        .aria_size_of_set(tab_count)
        .track_focus(&focus_handle)
        .focus_visible(|style| style.bg(rgba(palette.hover_surface)))
        .flex_auto()
        .min_w(px(metrics::TAB_MIN_WIDTH))
        .max_w(px(metrics::TAB_MAX_WIDTH))
        .items_center()
        .gap(px(metrics::TAB_CLOSE_GAP))
        .h(px(metrics::TAB_HEIGHT))
        .px(px(metrics::TAB_HORIZONTAL_PADDING))
        .rounded(px(metrics::CONTROL_RADIUS))
        .text_size(px(metrics::TAB_FONT_SIZE))
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

    control = if tab.active {
        control.bg(rgba(palette.surface))
    } else {
        control.hover(|style| style.bg(rgba(palette.hover_surface)))
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
    control = control.child(icon);

    control
        .child(div().flex_1().min_w_0().truncate().child(tab.label))
        .child(close_tab_button(
            close_id,
            close_label,
            tab.on_close,
            focus_index + 1 + if audio_control { 1 } else { 0 },
            palette,
        ))
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
) -> impl IntoElement {
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
        .text_color(rgb(palette.text_secondary))
        .hover(|style| style.bg(rgba(palette.hover_surface)))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(on_click)
        .child(close_icon(palette.text_secondary, metrics::TAB_ICON_SIZE))
}

fn new_tab_button(
    on_click: ClickHandler,
    focus_index: isize,
    palette: ThemeColors,
) -> impl IntoElement {
    h_stack()
        .id("browser-new-tab")
        .role(Role::Button)
        .aria_label("New tab")
        .tab_index(focus_index)
        .focus_visible(|style| style.border_1().border_color(rgb(palette.chosen)))
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .size(px(metrics::TAB_HEIGHT))
        .rounded(px(metrics::CONTROL_RADIUS))
        .text_color(rgb(palette.text_secondary))
        .hover(|style| style.bg(rgba(palette.hover_surface)))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(on_click)
        .child(add_icon(palette.text_secondary, metrics::TAB_ICON_SIZE))
}
