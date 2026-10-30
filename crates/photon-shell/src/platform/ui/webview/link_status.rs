//! The hovered link's address, shown over the page like other browsers' status bubble.

use gpui::{AnyElement, Context, div, prelude::*, px, relative, rgb, rgba};

use super::super::layout::{Elevated, Elevation, Raised};
use super::super::{metrics, theme::ThemeColors};
use super::PhotonWebView;

impl PhotonWebView {
    pub(in crate::platform) fn set_hovered_link(&mut self, url: String, cx: &mut Context<Self>) {
        let url = (!url.is_empty()).then_some(url);
        if self.hovered_link != url {
            self.hovered_link = url;
            cx.notify();
        }
    }

    pub(super) fn link_status(&self, palette: ThemeColors) -> Option<AnyElement> {
        let url = self.hovered_link.clone()?;
        Some(
            div()
                .absolute()
                .left(px(metrics::LINK_STATUS_INSET))
                .bottom(px(metrics::LINK_STATUS_INSET))
                .max_w(relative(metrics::LINK_STATUS_MAX_WIDTH_FRACTION))
                .px(px(metrics::CHIP_PADDING))
                .min_h(px(metrics::CHIP_HEIGHT))
                .rounded(px(metrics::chip_radius()))
                .border_1()
                .border_color(rgba(palette.menu_border))
                .raised(palette)
                .text_size(px(metrics::CHIP_FONT_SIZE))
                .text_color(rgb(palette.text_primary))
                .elevated(Elevation::Low)
                .truncate()
                .child(url)
                .into_any_element(),
        )
    }
}
