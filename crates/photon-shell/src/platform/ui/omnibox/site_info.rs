//! The active site's connection, the controls it has, and where its data and
//! permissions are managed. What the Engine cannot yet do shows dimmed.

use gpui::{AnyElement, FontWeight, Role, SharedString, div, prelude::*, px, rgb, rgba};

use super::super::button::{ButtonKind, ButtonSize, button, icon_button};
use super::super::controls::toggled;
use super::super::icons::{
    audio_icon, back_icon, camera_off_icon, close_icon, cookie_icon, lock_icon, microphone_icon,
    pop_out_icon, settings_icon,
};
use super::super::layout::{h_stack, v_stack};
use super::super::menu::{menu_separator, popover_surface};
use super::super::motion::{Edge, Transition, pressable};
use super::super::tooltip::label_tooltip;
use super::super::{ClickHandler, metrics, theme::ThemeColors};
use crate::platform::site_info::{
    BACK_LABEL, CLOSE_LABEL, ConnectionStatus, SiteControl, SiteDetail, SiteInfo, TITLE,
    VIEW_CERTIFICATE_LABEL,
};
use gpui_motion::PressScaleElement;

pub(super) struct SiteInfoActions {
    pub(super) close: ClickHandler,
    pub(super) toggle_sound: ClickHandler,
    pub(super) toggle_microphone: ClickHandler,
    /// Opens the connection's page, and returns from it.
    pub(super) show_security: ClickHandler,
    pub(super) back: ClickHandler,
}

pub(super) fn popover(
    info: SiteInfo,
    security_page: bool,
    actions: SiteInfoActions,
    width: f32,
    transition: Transition,
    palette: ThemeColors,
    cx: &gpui::App,
) -> impl IntoElement {
    // Only the page shown is built.
    let content = if security_page {
        security(&info, actions.back, palette).into_any_element()
    } else {
        site(&info, actions, palette, cx).into_any_element()
    };
    popover_surface(
        TITLE,
        Role::Dialog,
        width,
        Edge::Top,
        content,
        transition,
        palette,
    )
    .rounded(px(metrics::site_info_radius()))
    .p(px(metrics::SITE_INFO_PADDING))
}

/// The site's page: its controls, where its details are managed, and its
/// connection, which leads to the connection's page.
fn site(
    info: &SiteInfo,
    actions: SiteInfoActions,
    palette: ThemeColors,
    cx: &gpui::App,
) -> impl IntoElement {
    let motion = super::super::settings::Settings::motion(cx);
    let SiteInfoActions {
        close,
        toggle_sound,
        toggle_microphone,
        show_security,
        back: _,
    } = actions;
    let icon_size = metrics::SITE_INFO_TILE_ICON_SIZE;
    let ink = palette.text_primary;

    let mut toggle_microphone = Some(toggle_microphone);
    let mut toggle_sound = Some(toggle_sound);
    let controls =
        h_stack()
            .w_full()
            .gap(px(metrics::SITE_INFO_GAP))
            .children(SiteControl::ALL.map(|control| {
                let state = control.state(info);
                let on = state == Some(true);
                let on_toggle = state.and_then(|_| match control {
                    SiteControl::Microphone => toggle_microphone.take(),
                    SiteControl::Sound => toggle_sound.take(),
                    SiteControl::Camera | SiteControl::PopOut => None,
                });
                let icon = match control {
                    SiteControl::Camera => camera_off_icon(ink, icon_size).into_any_element(),
                    SiteControl::Microphone => {
                        microphone_icon("site-info-microphone-icon", ink, icon_size, on, &motion)
                            .into_any_element()
                    }
                    SiteControl::Sound => {
                        audio_icon("site-info-sound-icon", ink, icon_size, !on, &motion)
                            .into_any_element()
                    }
                    SiteControl::PopOut => pop_out_icon(ink, icon_size).into_any_element(),
                };
                tile(control, on, on_toggle, icon, palette)
            }));

    let details = v_stack()
        .gap(px(metrics::SITE_INFO_GAP))
        .child(heading(info.site.clone(), palette))
        .children(SiteDetail::ALL.map(|detail| {
            let icon = match detail {
                SiteDetail::Data => cookie_icon(ink, icon_size).into_any_element(),
                SiteDetail::Permissions => settings_icon(ink, icon_size).into_any_element(),
            };
            detail_row(detail, icon, palette)
        }));

    let status = ConnectionStatus::of(info);
    let connection = pressable(
        h_stack()
            .id("site-info-connection")
            .role(Role::Button)
            .aria_label(status.title())
            .items_center()
            .gap(px(metrics::SITE_INFO_GAP))
            .h(px(metrics::SITE_INFO_FOOTER_HEIGHT))
            .px(px(metrics::SITE_INFO_PADDING))
            .rounded(px(metrics::site_info_tile_radius()))
            .bg(rgba(palette.surface))
            .text_color(rgb(status_color(status, palette)))
            .font_weight(FontWeight::SEMIBOLD)
            .hover_style(|style| style.bg(rgba(palette.surface_hover)))
            .on_click(show_security)
            .child(connection_icon(status, palette))
            .child(status.label()),
    );
    let footer = h_stack()
        .w_full()
        .items_center()
        .justify_between()
        .child(connection)
        .child(
            icon_button(
                "site-info-close",
                CLOSE_LABEL,
                true,
                metrics::SITE_INFO_FOOTER_HEIGHT,
                close_icon(ink, icon_size),
                palette,
                close,
            )
            .bg(rgba(palette.surface)),
        );

    v_stack()
        .gap(px(metrics::SITE_INFO_SECTION_GAP))
        .child(controls)
        .child(details)
        .child(menu_separator(palette))
        .child(footer)
}

/// The connection's page: what its security means, with a way back.
fn security(info: &SiteInfo, back: ClickHandler, palette: ThemeColors) -> impl IntoElement {
    let status = ConnectionStatus::of(info);
    let header = h_stack()
        .w_full()
        .items_center()
        .gap(px(metrics::SITE_INFO_GAP))
        .child(
            icon_button(
                "site-info-back",
                BACK_LABEL,
                true,
                metrics::SITE_INFO_FOOTER_HEIGHT,
                back_icon(palette.text_primary, metrics::SITE_INFO_TILE_ICON_SIZE),
                palette,
                back,
            )
            .bg(rgba(palette.surface)),
        )
        .child(heading(status.title(), palette));
    let identity = h_stack()
        .items_center()
        .gap(px(metrics::SITE_INFO_GAP))
        .text_color(rgb(palette.text_secondary))
        .child(connection_icon(status, palette))
        .child(div().min_w_0().truncate().child(info.site.clone()));
    v_stack()
        .gap(px(metrics::SITE_INFO_SECTION_GAP))
        .child(header)
        .child(identity)
        .child(
            div()
                .w_full()
                .text_color(rgb(palette.text_primary))
                .child(status.summary()),
        )
        // Disabled until the Engine reports certificates; see
        // Documentation/Omnibox.md.
        .child(button(
            "site-info-certificate",
            VIEW_CERTIFICATE_LABEL,
            ButtonKind::Secondary,
            false,
            ButtonSize::Regular,
            palette,
            Box::new(|_, _, _| {}),
        ))
}

fn status_color(status: ConnectionStatus, palette: ThemeColors) -> u32 {
    match status {
        ConnectionStatus::Secure => palette.secure,
        ConnectionStatus::NotSecure => palette.error,
        ConnectionStatus::Unknown => palette.text_secondary,
    }
}

fn connection_icon(status: ConnectionStatus, palette: ThemeColors) -> impl IntoElement {
    lock_icon(
        status_color(status, palette),
        metrics::SITE_INFO_TILE_ICON_SIZE,
        status != ConnectionStatus::NotSecure,
    )
}

/// A square control for the site, chosen while `on`, and dimmed and inert
/// without `on_toggle`.
fn tile(
    control: SiteControl,
    on: bool,
    on_toggle: Option<ClickHandler>,
    icon: AnyElement,
    palette: ThemeColors,
) -> PressScaleElement<gpui::Stateful<gpui::Div>> {
    let enabled = on_toggle.is_some();
    let label = control.label();
    let tile = h_stack()
        .id(("site-info-control", control as usize))
        .role(Role::Switch)
        .aria_label(label)
        .aria_toggled(toggled(on))
        .aria_disabled(!enabled)
        .flex_1()
        .items_center()
        .justify_center()
        .h(px(metrics::SITE_INFO_TILE_HEIGHT))
        .rounded(px(metrics::site_info_tile_radius()))
        .bg(rgba(if on {
            palette.selected_surface
        } else {
            palette.surface
        }))
        .tooltip(label_tooltip(label, palette))
        .child(icon);
    let tile = match on_toggle {
        Some(on_toggle) => tile
            .hover_style(|style| style.bg(rgba(palette.surface_hover)))
            .on_click(on_toggle),
        None => tile.opacity(
            crate::platform::theme_config::theme()
                .opacity
                .disabled_control,
        ),
    };
    pressable(tile).enabled(enabled)
}

fn heading(label: impl Into<SharedString>, palette: ThemeColors) -> impl IntoElement {
    div()
        .w_full()
        .truncate()
        .text_size(px(metrics::SITE_INFO_HEADING_FONT_SIZE))
        .font_weight(FontWeight::BOLD)
        .text_color(rgb(palette.text_primary))
        .child(label.into())
}

/// A row with a round icon, a title and a quieter detail beneath it, dimmed
/// as the Engine cannot open it yet.
fn detail_row(detail: SiteDetail, icon: AnyElement, palette: ThemeColors) -> impl IntoElement {
    let title = detail.title();
    h_stack()
        .id(("site-info-detail", detail as usize))
        .aria_label(title)
        .aria_disabled(true)
        .items_center()
        .gap(px(metrics::SITE_INFO_GAP * 1.5))
        .w_full()
        .opacity(
            crate::platform::theme_config::theme()
                .opacity
                .disabled_control,
        )
        .child(
            h_stack()
                .flex_shrink_0()
                .items_center()
                .justify_center()
                .size(px(metrics::SITE_INFO_AVATAR_SIZE))
                .rounded_full()
                .bg(rgba(palette.surface))
                .child(icon),
        )
        .child(
            v_stack()
                .min_w_0()
                .child(
                    div()
                        .truncate()
                        .text_color(rgb(palette.text_primary))
                        .child(title),
                )
                .child(
                    div()
                        .truncate()
                        .text_size(px(metrics::SITE_INFO_DETAIL_FONT_SIZE))
                        .text_color(rgb(palette.text_secondary))
                        .child(detail.detail()),
                ),
        )
}
