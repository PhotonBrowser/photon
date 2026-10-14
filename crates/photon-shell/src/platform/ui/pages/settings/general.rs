//! The general section: whether the browser opens links from other apps,
//! and reopens its windows from last time.

use gpui::{App, prelude::*, px, rgb};
use std::time::Duration;

use super::super::super::button::{ButtonKind, ButtonSize, button};
use super::super::super::layout::v_stack;
use super::super::super::settings::Settings;
use super::super::super::{metrics, theme::ThemeColors};
use super::super::change;
use super::super::controls::{control_row, switch_row};
use super::super::layout::{group, secondary_text};
use crate::platform::app_update::update_app;
use crate::platform::{DefaultBrowserStatus, default_browser};

/// How long the page checks for the system's response.
const DEFAULT_BROWSER_CHECKS: u32 = 40;
const DEFAULT_BROWSER_CHECK_INTERVAL: Duration = Duration::from_millis(250);

pub(super) fn settings(palette: ThemeColors, cx: &App) -> impl IntoElement {
    v_stack()
        .gap(px(metrics::INTERNAL_PAGE_SECTION_GAP))
        .child(default_browser(palette))
        .child(config_file(palette, cx))
        .child(restore_session(palette, cx))
        .child(elastic_scroll(palette, cx))
}

fn config_file(palette: ThemeColors, cx: &App) -> impl IntoElement {
    let path = Settings::config_path(cx);
    let control = button(
        "settings-open-config",
        "Open config file",
        ButtonKind::Secondary,
        path.is_some(),
        ButtonSize::Regular,
        palette,
        Box::new(move |_, _, _| {
            if let Some(path) = &path {
                #[cfg(target_os = "macos")]
                let (program, args) = ("open", vec![path.as_os_str()]);
                #[cfg(target_os = "linux")]
                let (program, args) = ("xdg-open", vec![path.as_os_str()]);
                #[cfg(any(target_os = "macos", target_os = "linux"))]
                {
                    let _ = std::process::Command::new(program).args(args).spawn();
                }
            }
        }),
    );
    v_stack()
        .gap(px(metrics::MENU_ITEM_GAP))
        .child(group(palette).child(control_row("Configuration", control)))
        .child(secondary_text(
            "Edit browser preferences in config.json.",
            palette,
        ))
}

fn elastic_scroll(palette: ThemeColors, cx: &App) -> impl IntoElement {
    let enabled = Settings::get(cx).elastic_scroll;
    v_stack()
        .gap(px(metrics::MENU_ITEM_GAP))
        .child(group(palette).child(switch_row(
            "settings-elastic-scroll",
            "Elastic scrolling",
            enabled,
            palette,
            change(move |settings| settings.elastic_scroll = !enabled),
        )))
        .child(secondary_text(
            "Let pages stretch briefly when you scroll past an edge.",
            palette,
        ))
}

fn restore_session(palette: ThemeColors, cx: &App) -> impl IntoElement {
    let restore = Settings::get(cx).restore_session;
    v_stack()
        .gap(px(metrics::MENU_ITEM_GAP))
        .child(group(palette).child(switch_row(
            "settings-restore-session",
            "Reopen windows and tabs",
            restore,
            palette,
            change(move |settings| settings.restore_session = !restore),
        )))
        .child(secondary_text(
            format!(
                "When {} opens, it brings back the windows and tabs open when it last quit.",
                photon_brand::NAME
            ),
            palette,
        ))
}

fn default_browser(palette: ThemeColors) -> impl IntoElement {
    let name = photon_brand::NAME;
    let (control, note) = match default_browser::status() {
        DefaultBrowserStatus::Default => (
            div_text("In use", palette).into_any_element(),
            format!("Links you open in other apps open in {name}."),
        ),
        DefaultBrowserStatus::NotDefault => (
            button(
                "settings-make-default-browser",
                "Make default",
                ButtonKind::Secondary,
                true,
                ButtonSize::Regular,
                palette,
                Box::new(|_, _, cx| make_default_browser(cx)),
            )
            .into_any_element(),
            format!(
                "Make {name} open links from other apps. The operating system will ask you to confirm."
            ),
        ),
        DefaultBrowserStatus::Unavailable => (
            div_text("Unavailable", palette).into_any_element(),
            "This build can't change the system's default browser setting.".to_string(),
        ),
    };
    v_stack()
        .gap(px(metrics::MENU_ITEM_GAP))
        .child(group(palette).child(control_row("Default browser", control)))
        .child(secondary_text(note, palette))
}

fn div_text(text: &'static str, palette: ThemeColors) -> impl IntoElement {
    gpui::div()
        .text_color(rgb(palette.text_secondary))
        .child(text)
}

/// Requests a system-level change, then redraws while the system responds.
fn make_default_browser(cx: &mut App) {
    default_browser::request();
    cx.spawn(async move |cx| {
        for _ in 0..DEFAULT_BROWSER_CHECKS {
            cx.background_executor()
                .timer(DEFAULT_BROWSER_CHECK_INTERVAL)
                .await;
            let answered = default_browser::status() == DefaultBrowserStatus::Default;
            update_app(cx, |cx| cx.refresh_windows()).await;
            if answered {
                break;
            }
        }
    })
    .detach();
}
