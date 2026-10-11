//! The general section: whether the browser opens links from other apps,
//! and reopens its windows from last time.

use gpui::{App, prelude::*, px, rgb};
use std::time::Duration;

use super::super::super::super::default_browser::DefaultBrowser;
use super::super::super::button::{ButtonKind, ButtonSize, button};
use super::super::super::layout::v_stack;
use super::super::super::settings::Settings;
use super::super::super::{metrics, theme::ThemeColors};
use super::super::change;
use super::super::controls::{control_row, switch_row};
use super::super::layout::{group, secondary_text};
use crate::platform::app_update::update_app;

/// How long the page keeps checking for the person's answer to macOS.
const DEFAULT_BROWSER_CHECKS: u32 = 40;
const DEFAULT_BROWSER_CHECK_INTERVAL: Duration = Duration::from_millis(250);

pub(super) fn settings(palette: ThemeColors, cx: &App) -> impl IntoElement {
    v_stack()
        .gap(px(metrics::INTERNAL_PAGE_SECTION_GAP))
        .child(default_browser(palette))
        .child(restore_session(palette, cx))
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
    let (control, note) = match DefaultBrowser::current() {
        DefaultBrowser::Default => (
            div_text("In use", palette).into_any_element(),
            format!("Links you open in other apps open in {name}."),
        ),
        DefaultBrowser::NotDefault => (
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
            format!("Make {name} open links from other apps. macOS asks you to confirm."),
        ),
        DefaultBrowser::Unavailable => (
            div_text("Unavailable", palette).into_any_element(),
            format!("{name} can become the default browser once it is opened as an app."),
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

/// Asks macOS, then redraws while the person answers its prompt.
fn make_default_browser(cx: &mut App) {
    DefaultBrowser::request();
    cx.spawn(async move |cx| {
        for _ in 0..DEFAULT_BROWSER_CHECKS {
            cx.background_executor()
                .timer(DEFAULT_BROWSER_CHECK_INTERVAL)
                .await;
            let answered = DefaultBrowser::current() == DefaultBrowser::Default;
            update_app(cx, |cx| cx.refresh_windows()).await;
            if answered {
                break;
            }
        }
    })
    .detach();
}
