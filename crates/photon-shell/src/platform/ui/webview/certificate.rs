//! Warning about a page whose certificate could not be verified, with the
//! choice to go back or, knowingly, proceed.

use gpui::{AnyElement, Context, FontWeight, Role, div, prelude::*, px, rgb};
use photon_core::BrowserCommand;

use super::super::super::trace;
use super::super::button::{ButtonKind, ButtonSize, button};
use super::super::controls::OwnsClicks;
use super::super::icons::warning_icon;
use super::super::layout::{h_stack, secondary_text, v_stack};
use super::super::pages::page_frame;
use super::super::{ClickHandler, metrics, theme::ThemeColors};
use super::PhotonWebView;

/// A page that failed to load because its certificate could not be verified.
#[derive(Clone, Debug, PartialEq)]
pub(in crate::platform) struct CertificateWarning {
    url: String,
    host: String,
    /// Whether the details and the way to proceed are shown.
    details_shown: bool,
}

impl PhotonWebView {
    pub(in crate::platform) fn certificate_error(
        &mut self,
        url: String,
        host: String,
        cx: &mut Context<Self>,
    ) {
        self.certificate_warning = Some(CertificateWarning {
            url,
            host,
            details_shown: false,
        });
        self.state_changed(cx);
    }

    /// Any navigation leaves the warning behind.
    pub(super) fn dismiss_certificate_warning(&mut self) {
        self.certificate_warning = None;
    }

    fn leave_certificate_warning(&mut self, cx: &mut Context<Self>) {
        let command = if self.state.can_go_back {
            BrowserCommand::Back
        } else {
            BrowserCommand::Navigate("about:blank".to_owned())
        };
        if let Err(error) = self.execute(command, cx) {
            trace(format_args!("leaving certificate warning: {error:#}"));
        }
    }

    fn proceed_past_certificate_warning(&mut self, cx: &mut Context<Self>) {
        let Some(warning) = self.certificate_warning.take() else {
            return;
        };
        self.session
            .runtime()
            .allow_certificate_errors_for_host(&warning.host);
        if let Err(error) = self.execute(BrowserCommand::Navigate(warning.url), cx) {
            trace(format_args!(
                "proceeding past certificate warning: {error:#}"
            ));
        }
    }

    pub(super) fn certificate_overlay(
        &self,
        palette: ThemeColors,
        backdrop: u32,
        radius: f32,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        let warning = self.certificate_warning.as_ref()?;
        let host = warning.host.clone();
        let back: ClickHandler = Box::new(cx.listener(|this, _, _, cx| {
            this.leave_certificate_warning(cx);
        }));
        let toggle_details: ClickHandler = Box::new(cx.listener(|this, _, _, cx| {
            if let Some(warning) = this.certificate_warning.as_mut() {
                warning.details_shown = !warning.details_shown;
                cx.notify();
            }
        }));
        let mut content = v_stack()
            .w_full()
            .max_w(px(metrics::INTERNAL_PAGE_MAX_WIDTH))
            .items_center()
            .gap(px(metrics::INTERNAL_PAGE_GROUP_GAP))
            .child(warning_icon(
                palette.error,
                metrics::INTERNAL_PAGE_ICON_SIZE,
            ))
            .child(
                div()
                    .id("certificate-warning-title")
                    .role(Role::Heading)
                    .aria_level(1)
                    .text_size(px(metrics::INTERNAL_PAGE_SECTION_SIZE))
                    .font_weight(FontWeight::SEMIBOLD)
                    .text_color(rgb(palette.text_primary))
                    .text_center()
                    .child("Your connection isn\u{2019}t private"),
            )
            .child(
                secondary_text(
                    format!(
                        "Someone might be trying to steal your information from {host}, \
                         such as passwords, messages or card details."
                    ),
                    palette,
                )
                .text_center(),
            )
            .child(
                h_stack()
                    .gap(px(metrics::LIST_ROW_GAP))
                    .child(button(
                        "certificate-warning-details",
                        if warning.details_shown {
                            "Hide Details"
                        } else {
                            "Details"
                        },
                        ButtonKind::Secondary,
                        true,
                        ButtonSize::Regular,
                        palette,
                        toggle_details,
                    ))
                    .child(button(
                        "certificate-warning-back",
                        "Go Back",
                        ButtonKind::Primary,
                        true,
                        ButtonSize::Regular,
                        palette,
                        back,
                    )),
            );
        if warning.details_shown {
            let proceed: ClickHandler = Box::new(cx.listener(|this, _, _, cx| {
                this.proceed_past_certificate_warning(cx);
            }));
            content = content
                .child(
                    secondary_text(
                        format!(
                            "The certificate for {host} couldn\u{2019}t be verified. It may have \
                             expired, be self-signed, or belong to another site. Only continue \
                             if you trust this network and site."
                        ),
                        palette,
                    )
                    .text_center(),
                )
                .child(button(
                    "certificate-warning-proceed",
                    format!("Continue to {host} (unsafe)"),
                    ButtonKind::Destructive,
                    true,
                    ButtonSize::Regular,
                    palette,
                    proceed,
                ));
        }
        Some(
            div()
                .absolute()
                .inset_0()
                // The page frame is translucent, and the engine's own error page is beneath.
                .rounded(px(radius))
                .bg(rgb(backdrop))
                .child(page_frame(
                    v_stack()
                        .id("page-certificate-warning")
                        .size_full()
                        .items_center()
                        .justify_center()
                        .overflow_y_scroll()
                        .p(px(metrics::INTERNAL_PAGE_INSET))
                        .role(Role::Group)
                        .aria_label("Certificate warning")
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
