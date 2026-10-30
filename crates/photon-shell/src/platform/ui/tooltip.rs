//! Small, themed tooltips for icon-only controls.

use gpui::{AnyView, App, Context, Render, SharedString, Window, div, prelude::*, px, rgb, rgba};

use super::layout::{Elevated, Elevation, Raised};
use super::{metrics, theme::ThemeColors};

struct LabelTooltip {
    label: SharedString,
    palette: ThemeColors,
}

impl Render for LabelTooltip {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .px(px(metrics::TOOLTIP_HORIZONTAL_PADDING))
            .py(px(metrics::TOOLTIP_VERTICAL_PADDING))
            .rounded(px(metrics::tooltip_radius()))
            .border_1()
            .border_color(rgba(self.palette.menu_border))
            .raised(self.palette)
            .elevated(Elevation::High)
            .text_size(px(metrics::TOOLTIP_FONT_SIZE))
            .text_color(rgb(self.palette.raised_text_primary))
            .child(self.label.clone())
    }
}

pub(super) fn label_tooltip(
    label: impl Into<SharedString>,
    palette: ThemeColors,
) -> impl Fn(&mut Window, &mut App) -> AnyView + 'static {
    let label = label.into();
    move |_, cx| {
        cx.new(|_| LabelTooltip {
            label: label.clone(),
            palette,
        })
        .into()
    }
}
