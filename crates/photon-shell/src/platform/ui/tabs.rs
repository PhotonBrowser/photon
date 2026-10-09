//! Browser tab strip and its controls.

use gpui::{
    Animation, AnimationExt, AnyElement, App, ClickEvent, ElementId, FocusHandle, Image,
    ImageFormat, ImageSource, KeyDownEvent, MouseButton, ObjectFit, Role, Window, div, img,
    prelude::*, px, rgb, rgba,
};
use std::sync::{Arc, LazyLock};
use std::time::Duration;

use super::Favicon;
use super::icons::{add_icon, close_icon, globe_icon, loading_spinner};
use super::layout::h_stack;
use super::{metrics, theme::ThemeColors};

type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>;

/// What a tab shows before its title.
pub(super) enum TabIcon {
    /// The page is loading; the value is the spinner's animation step.
    Loading(usize),
    Favicon(Favicon),
    /// A page without an icon of its own.
    Page,
    /// A new tab, which shows the Photon logo.
    NewTab,
}

/// Identifies an icon a tab reveals with the appear animation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum RevealedIcon {
    NewTab,
    Favicon(u64),
}

impl TabIcon {
    /// The icon the appear animation reveals. The spinner and the generic
    /// page icon are placeholders, so they neither animate nor replace the
    /// last revealed icon: a page's icon that returns after them stays put.
    pub(super) fn revealed(&self) -> Option<RevealedIcon> {
        match self {
            Self::NewTab => Some(RevealedIcon::NewTab),
            Self::Favicon(favicon) => Some(RevealedIcon::Favicon(favicon.key)),
            Self::Loading(_) | Self::Page => None,
        }
    }
}

/// How long a tab icon takes to grow to full size when it appears.
pub(super) const ICON_APPEAR_DURATION: Duration = Duration::from_millis(260);
/// The fraction of full size an appearing tab icon starts from.
const ICON_APPEAR_START_SCALE: f32 = 0.35;

static NEW_TAB_LOGO: LazyLock<Arc<Image>> = LazyLock::new(|| {
    Arc::new(Image::from_bytes(
        ImageFormat::Svg,
        include_bytes!("../../../assets/monotone-planet.svg").to_vec(),
    ))
});

/// Eases out past full size and settles back, so icons pop in.
fn ease_out_back(delta: f32) -> f32 {
    const OVERSHOOT: f32 = 1.70158;
    let t = delta - 1.0;
    1.0 + (OVERSHOOT + 1.0) * t * t * t + OVERSHOOT * t * t
}

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
        .child(icon.with_animation(
            id,
            Animation::new(ICON_APPEAR_DURATION).with_easing(ease_out_back),
            move |icon, delta| {
                let scale = ICON_APPEAR_START_SCALE + (1.0 - ICON_APPEAR_START_SCALE) * delta;
                icon.size(px(size * scale))
            },
        ))
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
        .min_w(px(0.0))
        .overflow_x_scroll();

    for (index, tab) in tabs.into_iter().enumerate() {
        tab_list = tab_list.child(browser_tab(
            tab,
            index + 1,
            tab_count,
            (index * 2) as isize,
            palette,
        ));
    }

    h_stack()
        .w_full()
        .items_center()
        .gap(px(metrics::TAB_STRIP_GAP))
        .h(px(metrics::TITLEBAR_HEIGHT))
        .px(px(metrics::TAB_STRIP_INSET))
        .tab_group()
        .child(tab_list)
        .child(new_tab_button(
            on_new_tab,
            (tab_count * 2) as isize,
            palette,
        ))
}

fn browser_tab(
    tab: TabItem,
    position: usize,
    tab_count: usize,
    focus_index: isize,
    palette: ThemeColors,
) -> impl IntoElement {
    let close_label = format!("Close {}", tab.label);
    let close_id = format!("{}-close", tab.id);
    let tab_id = tab.id.clone();
    let focus_handle = tab.focus_handle.tab_index(focus_index).tab_stop(tab.active);
    let mut control = h_stack()
        .id(tab.id)
        .role(Role::Tab)
        .aria_label(tab.label.clone())
        .aria_selected(tab.active)
        .aria_position_in_set(position)
        .aria_size_of_set(tab_count)
        .track_focus(&focus_handle)
        .focus_visible(|style| style.border_1().border_color(rgb(palette.accent)))
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
        .on_key_down(tab.on_key_down);

    control = if tab.active {
        control.bg(rgba(palette.tab_active_surface))
    } else {
        control.hover(|style| style.bg(rgba(palette.tab_hover_surface)))
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
        TabIcon::Favicon(favicon) => sized_icon(
            img(ImageSource::Render(favicon.image)).object_fit(ObjectFit::Contain),
            icon_id,
            tab.icon_appearing,
            icon_size,
        ),
        TabIcon::NewTab => sized_icon(
            img(NEW_TAB_LOGO.clone()).object_fit(ObjectFit::Contain),
            icon_id,
            tab.icon_appearing,
            icon_size,
        ),
    };
    control = control.child(icon);

    control
        .child(div().flex_1().min_w(px(0.0)).truncate().child(tab.label))
        .child(close_tab_button(
            close_id,
            close_label,
            tab.on_close,
            focus_index + 1,
            palette,
        ))
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
        .focus_visible(|style| style.border_1().border_color(rgb(palette.accent)))
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .size(px(metrics::TAB_CLOSE_BUTTON_SIZE))
        .rounded(px(metrics::CONTROL_RADIUS))
        .text_color(rgb(palette.text_secondary))
        .hover(|style| style.bg(rgba(palette.control_hover_surface)))
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
        .focus_visible(|style| style.border_1().border_color(rgb(palette.accent)))
        .flex_shrink_0()
        .items_center()
        .justify_center()
        .size(px(metrics::TAB_HEIGHT))
        .rounded(px(metrics::CONTROL_RADIUS))
        .text_color(rgb(palette.text_secondary))
        .hover(|style| style.bg(rgba(palette.control_hover_surface)))
        .on_mouse_down(MouseButton::Left, |_, _, cx| cx.stop_propagation())
        .on_click(on_click)
        .child(add_icon(palette.text_secondary, metrics::TAB_ICON_SIZE))
}
