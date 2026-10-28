//! The active site's connection, the controls it has, and where its data and
//! permissions are managed. What the Engine cannot yet do shows dimmed.

use gpui::{AnyElement, Role, SharedString, prelude::*, px, rgb};
use std::cell::Cell;
use std::rc::Rc;

use super::super::button::toolbar_button;
use super::super::controls::{haptic_switch_toggle, switch};
use super::super::icons::{
    audio_icon, camera_off_icon, chevron_right_icon, close_icon, cookie_icon, external_icon,
    lock_icon, microphone_icon, pop_out_icon, settings_icon,
};
use super::super::layout::{h_stack, v_stack};
use super::super::menu::{menu_separator, popover_surface};
use super::super::motion::Transition;
use super::super::{ClickHandler, metrics, theme::ThemeColors};

/// What the popover shows about the active site.
pub(super) struct SiteInfo {
    pub(super) site: String,
    pub(super) connection_secure: Option<bool>,
    /// Whether the page may play sound; `None` before it has a page.
    pub(super) sound_on: Option<bool>,
    /// Whether the site may use the microphone; `None` without a site.
    pub(super) microphone_on: Option<bool>,
}

pub(super) struct SiteInfoActions {
    pub(super) close: ClickHandler,
    pub(super) toggle_sound: ClickHandler,
    pub(super) toggle_microphone: ClickHandler,
}

pub(super) fn popover(
    info: SiteInfo,
    actions: SiteInfoActions,
    width: f32,
    transition: Transition,
    palette: ThemeColors,
    cx: &gpui::App,
) -> impl IntoElement {
    let SiteInfoActions {
        close,
        toggle_sound,
        toggle_microphone,
    } = actions;
    let header = h_stack()
        .items_center()
        .w_full()
        .pl(px(metrics::MENU_ITEM_HORIZONTAL_PADDING))
        .child(
            gpui::div()
                .flex_1()
                .min_w_0()
                .text_size(px(metrics::SITE_INFO_TITLE_FONT_SIZE))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(rgb(palette.text_primary))
                .truncate()
                .child(info.site),
        )
        .child(toolbar_button(
            "site-info-close",
            "Close",
            0,
            true,
            close_icon(palette.text_secondary, metrics::SITE_INFO_ICON_SIZE).into_any_element(),
            palette,
            close,
        ));

    let (status, status_color) = match info.connection_secure {
        Some(true) => ("Connection is secure", palette.secure),
        Some(false) => ("Connection is not secure", palette.error),
        None => ("No connection details", palette.text_secondary),
    };
    // The Engine reports only whether the connection is secure, not its
    // certificate, so there are no details to lead to yet.
    let connection = row(
        "site-info-connection",
        lock_icon(
            status_color,
            metrics::SITE_INFO_ICON_SIZE,
            info.connection_secure != Some(false),
        )
        .into_any_element(),
        status,
        status_color,
        disabled_trailing(chevron_right_icon(
            palette.text_secondary,
            metrics::SITE_INFO_ICON_SIZE,
        )),
        false,
    );

    let sound_on = info.sound_on.unwrap_or(true);
    let controls = v_stack()
        .child(switch_row(
            "site-info-camera",
            camera_off_icon(palette.text_secondary, metrics::SITE_INFO_ICON_SIZE)
                .into_any_element(),
            "Camera",
            false,
            None,
            palette,
        ))
        .child(switch_row(
            "site-info-microphone",
            microphone_icon(
                palette.text_secondary,
                metrics::SITE_INFO_ICON_SIZE,
                info.microphone_on == Some(true),
            )
            .into_any_element(),
            "Microphone",
            info.microphone_on == Some(true),
            info.microphone_on.map(|_| toggle_microphone),
            palette,
        ))
        .child(switch_row(
            "site-info-sound",
            audio_icon(
                palette.text_secondary,
                metrics::SITE_INFO_ICON_SIZE,
                !sound_on,
            )
            .into_any_element(),
            "Sound",
            sound_on,
            info.sound_on.map(|_| toggle_sound),
            palette,
        ))
        .child(switch_row(
            "site-info-pop-out",
            pop_out_icon(palette.text_secondary, metrics::SITE_INFO_ICON_SIZE).into_any_element(),
            "Pop out view",
            false,
            None,
            palette,
        ));

    let site_data = v_stack()
        .child(row(
            "site-info-data",
            cookie_icon(palette.text_secondary, metrics::SITE_INFO_ICON_SIZE).into_any_element(),
            "Data and cookies",
            palette.text_primary,
            chevron_right_icon(palette.text_secondary, metrics::SITE_INFO_ICON_SIZE)
                .into_any_element(),
            true,
        ))
        .child(row(
            "site-info-permissions",
            settings_icon(palette.text_secondary, metrics::SITE_INFO_ICON_SIZE).into_any_element(),
            "Permissions",
            palette.text_primary,
            external_icon(palette.text_secondary, metrics::SITE_INFO_ICON_SIZE).into_any_element(),
            true,
        ));

    let content = v_stack()
        .gap(px(metrics::MENU_ITEM_GAP))
        .child(header)
        .child(connection)
        .child(menu_separator(palette))
        .child(controls)
        .child(menu_separator(palette))
        .child(site_data);

    popover_surface(
        "Site information",
        Role::Dialog,
        width,
        content,
        transition,
        palette,
        cx,
    )
}

/// A row with a leading icon, a label and something at its end. A disabled
/// row is dimmed and announced as unavailable.
fn row(
    id: &'static str,
    icon: AnyElement,
    label: impl Into<SharedString>,
    label_color: u32,
    trailing: AnyElement,
    disabled: bool,
) -> gpui::Stateful<gpui::Div> {
    let label = label.into();
    let opacity = if disabled {
        crate::platform::theme_config::theme()
            .opacity
            .disabled_control
    } else {
        1.0
    };
    h_stack()
        .id(id)
        .aria_label(label.clone())
        .aria_disabled(disabled)
        .items_center()
        .gap(px(metrics::MENU_ITEM_GAP * 4.0))
        .w_full()
        .h(px(metrics::SITE_INFO_ROW_HEIGHT))
        .px(px(metrics::MENU_ITEM_HORIZONTAL_PADDING))
        .opacity(opacity)
        .child(icon)
        .child(
            gpui::div()
                .flex_1()
                .min_w_0()
                .truncate()
                .text_color(rgb(label_color))
                .child(label),
        )
        .child(trailing)
}

/// A row whose switch turns `label` on or off, disabled without `on_toggle`.
fn switch_row(
    id: &'static str,
    icon: AnyElement,
    label: &'static str,
    on: bool,
    on_toggle: Option<ClickHandler>,
    palette: ThemeColors,
) -> impl IntoElement {
    let disabled = on_toggle.is_none();
    let on_toggle: Rc<ClickHandler> = Rc::new(on_toggle.unwrap_or_else(|| Box::new(|_, _, _| {})));
    let on_activate = haptic_switch_toggle(on_toggle.clone(), Rc::new(Cell::new(false)));
    let control = switch(
        id,
        on,
        palette,
        Some(label),
        on_toggle,
        on_activate,
        Rc::new(Cell::new(false)),
    )
    .disabled(disabled);
    // The switch dims itself, so the row dims only its icon and label.
    h_stack()
        .items_center()
        .w_full()
        .pr(px(metrics::MENU_ITEM_HORIZONTAL_PADDING))
        .child(
            row(
                id,
                icon,
                label,
                palette.text_primary,
                gpui::div().into_any_element(),
                disabled,
            )
            .flex_1(),
        )
        .child(control)
}

/// An end mark dimmed for a detail the Engine cannot show yet.
fn disabled_trailing(mark: impl IntoElement) -> AnyElement {
    gpui::div()
        .opacity(
            crate::platform::theme_config::theme()
                .opacity
                .disabled_control,
        )
        .child(mark)
        .into_any_element()
}
