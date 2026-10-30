//! Recovering a tab whose page process crashed.

use gpui::{AnyElement, Context, FontWeight, Role, div, prelude::*, px, rgb};
use photon_core::{BrowserCommand, CrashResponse};
use std::time::Instant;

use super::super::super::trace;
use super::super::button::{ButtonKind, ButtonSize, button};
use super::super::controls::OwnsClicks;
use super::super::layout::{secondary_text, v_stack};
use super::super::pages::page_frame;
use super::super::{ClickHandler, metrics, theme::ThemeColors};
use super::PhotonWebView;

/// What a tab says about recovering from a crash.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::platform) enum CrashNotice {
    /// The page crashed and is reloading in a fresh process.
    Reloading,
    /// The reloaded page showed at this time.
    Reloaded(Instant),
    /// The page crashed again soon after recovering, so it was not reloaded.
    KeepsCrashing { failed_url: Option<String> },
}

impl PhotonWebView {
    /// The page's process crashed: reloads it, unless it keeps crashing.
    pub(in crate::platform) fn handle_engine_crash(
        &mut self,
        failed_url: Option<String>,
        cx: &mut Context<Self>,
    ) {
        // The replaced page process no longer waits for its dialog.
        self.dialogs.reset();
        let response = self.crashes.crashed(Instant::now());
        trace(format_args!("page crashed: {response:?}"));
        match response {
            CrashResponse::Reload => {
                self.crash_notice = Some(CrashNotice::Reloading);
                if let Err(error) = self.session.execute(BrowserCommand::Reload) {
                    trace(format_args!("reloading crashed page: {error:#}"));
                }
            }
            CrashResponse::GiveUp => {
                self.crash_notice = Some(CrashNotice::KeepsCrashing { failed_url });
            }
        }
        self.state_changed(cx);
    }

    /// The page that replaced a crashed one is showing.
    pub(in crate::platform) fn crash_recovered(&mut self, cx: &mut Context<Self>) {
        let now = Instant::now();
        self.crashes.recovered(now);
        trace(format_args!("crashed page recovered"));
        if self.crash_notice == Some(CrashNotice::Reloading) {
            self.crash_notice = Some(CrashNotice::Reloaded(now));
            self.state_changed(cx);
        }
    }

    /// Replaces a page that has crashed repeatedly with a persistent recovery
    /// surface. The first crash still gets an automatic reload.
    pub(super) fn crash_overlay(
        &self,
        palette: ThemeColors,
        radius: f32,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let Some(CrashNotice::KeepsCrashing { failed_url }) = self.crash_notice.as_ref() else {
            return None;
        };
        let failed_address = failed_url
            .as_deref()
            .filter(|url| !url.is_empty() && *url != "unknown page")
            .map(photon_core::display_address);
        let reload: ClickHandler = Box::new(cx.listener(|this, _, _, cx| {
            if let Err(error) = this.execute(BrowserCommand::Reload, cx) {
                trace(format_args!("reloading crashed page: {error:#}"));
            }
        }));
        let mut content = v_stack()
            .w_full()
            .max_w(px(metrics::INTERNAL_PAGE_MAX_WIDTH))
            .items_center()
            .gap(px(metrics::INTERNAL_PAGE_GROUP_GAP))
            .child(
                div()
                    .id("crash-page-title")
                    .role(Role::Heading)
                    .aria_level(1)
                    .text_size(px(metrics::INTERNAL_PAGE_SECTION_SIZE))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(palette.text_primary))
                    .text_center()
                    .child("This page keeps crashing"),
            )
            .child(
                secondary_text(
                    "Photon stopped reloading this page to avoid a crash loop. Reload it to try again.",
                    palette,
                )
                .text_center(),
            );
        if let Some(address) = failed_address {
            content = content.child(
                div()
                    .id("crash-failed-url")
                    .w_full()
                    .text_size(px(metrics::INTERNAL_PAGE_BODY_SIZE))
                    .text_color(rgb(palette.text_secondary))
                    .text_center()
                    .truncate()
                    .aria_label(address.clone())
                    .child(address),
            );
        }
        content = content.child(button(
            "crash-reload-page",
            "Reload page",
            ButtonKind::Primary,
            true,
            ButtonSize::Regular,
            palette,
            reload,
        ));
        Some(
            div()
                .absolute()
                .inset_0()
                .child(page_frame(
                    v_stack()
                        .id("page-crash-recovery")
                        .size_full()
                        .items_center()
                        .justify_center()
                        .overflow_y_scroll()
                        .p(px(metrics::INTERNAL_PAGE_INSET))
                        .role(Role::Group)
                        .aria_label("Page crash recovery")
                        .aria_live(gpui::accesskit::Live::Assertive)
                        .occlude()
                        .owns_clicks()
                        .on_key_down(|_, _, cx| cx.stop_propagation())
                        .on_key_up(|_, _, cx| cx.stop_propagation())
                        .child(content)
                        .into_any_element(),
                    palette,
                    radius,
                ))
                .into_any_element(),
        )
    }
}
