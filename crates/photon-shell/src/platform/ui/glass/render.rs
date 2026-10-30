//! Draws a [`Glass`]: frosted, over native glass, or as its plain fallback.

use gpui::{App, Div, ElementId, IntoElement, RenderOnce, Stateful, Styled, Window, px, rgba};

use super::super::metrics;
use super::super::motion::AnimateIn;
use super::super::settings::Settings;
use super::super::theme::{ThemeColors, with_opacity};
use super::Glass;
#[cfg(target_os = "macos")]
use super::{
    glass_available,
    macos::{NativeGlass, native_glass},
};

impl RenderOnce for Glass {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Glass {
            id,
            content,
            enabled,
            tint,
            backing,
            interactive,
            frosted,
            motion,
            fallback,
        } = self;
        let motion_config = Settings::motion(cx);
        // A stable id keeps the content's keyed state as it closes.
        let animated = |styled: Stateful<Div>, id: &ElementId| match motion {
            Some((entrance, transition)) => styled.animate_with_offset_stable_id(
                (id.clone(), "motion"),
                entrance,
                transition,
                &motion_config,
                (0.0, 0.0),
            ),
            None => styled.into_any_element(),
        };

        if let Some(palette) = frosted
            && enabled
            && Settings::get(cx).glass_surfaces
        {
            return animated(frosted_surface(content, palette), &id);
        }

        #[cfg(target_os = "macos")]
        if enabled
            && frosted.is_none()
            && glass_available(cx)
            && let Some(backdrop) = native_glass(&id, window, cx)
        {
            let glass = NativeGlass {
                tint,
                backing,
                interactive,
            };
            return glass.under(backdrop, content, window, |content| animated(content, &id));
        }
        // Elsewhere there is no native glass, only frosted glass or the
        // painted fallback, which look the same without the system material.
        #[cfg(not(target_os = "macos"))]
        let _ = (&tint, backing, interactive, &window);

        let content = match fallback {
            Some(fallback) => fallback(content),
            None => content,
        };
        animated(content, &id)
    }
}

/// GPUI's own glass: a blur of what lies behind under a translucent surface.
fn frosted_surface(content: Stateful<Div>, palette: ThemeColors) -> Stateful<Div> {
    let opacity = crate::platform::theme_config::theme().opacity.frosted_glass;
    content
        .backdrop_blur(px(metrics::FROSTED_GLASS_BLUR))
        .bg(rgba(with_opacity(palette.menu_surface, opacity)))
        .border_1()
        .border_color(rgba(palette.menu_border))
}
