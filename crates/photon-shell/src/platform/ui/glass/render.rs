//! Draws a [`Glass`]: frosted, over native glass, or as its plain fallback.

use gpui::{
    App, Div, ElementId, IntoElement, RenderOnce, Stateful, Styled, Window, prelude::*, px, rgba,
};
#[cfg(target_os = "macos")]
use gpui::{Length, Pixels, StyleRefinement, div};

use super::super::metrics;
use super::super::motion::AnimateIn;
use super::super::settings::Settings;
use super::super::theme::{ThemeColors, with_opacity};
use super::Glass;
#[cfg(target_os = "macos")]
use super::glass_available;

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

/// How native glass lies beneath a [`Glass`]'s content, on macOS.
#[cfg(target_os = "macos")]
struct NativeGlass {
    tint: Vec<u32>,
    backing: Option<u32>,
    interactive: bool,
}

#[cfg(target_os = "macos")]
impl NativeGlass {
    /// Draws `content`, animated by `animate`, over `backdrop`. The glass
    /// and the content are siblings, so the content's own background and
    /// border draw over the glass; a wrapper takes the content's place in its
    /// parent's layout. The glass stays still: it draws nothing while it
    /// changes.
    fn under(
        self,
        backdrop: crate::platform::AppKitGlassBackdrop,
        mut content: Stateful<Div>,
        window: &Window,
        animate: impl FnOnce(Stateful<Div>) -> gpui::AnyElement,
    ) -> gpui::AnyElement {
        let mut placement = div();
        move_placement(content.style(), placement.style());
        let glass = backdrop
            .corner_radius(corner_radius(content.style(), window))
            .window_tint(crate::platform::GlassWindowTint {
                stops: if self.backing.is_some() {
                    Vec::new()
                } else {
                    self.tint
                },
                angle_degrees: crate::platform::theme_config::theme()
                    .window
                    .gradient_angle_degrees,
            })
            .backing(self.backing)
            .interactive(self.interactive)
            .absolute()
            .inset_0();
        placement
            .child(glass)
            .child(animate(content))
            .into_any_element()
    }
}

/// The glass follows the element's top-left corner radius.
#[cfg(target_os = "macos")]
fn corner_radius(style: &StyleRefinement, window: &Window) -> Pixels {
    style
        .corner_radii
        .top_left
        .map(|radius| radius.to_pixels(window.rem_size()))
        .unwrap_or_default()
}

/// Moves where an element sits in its parent from `content` to `wrapper`:
/// its position, insets, margins, flex sizing and size. The content then
/// fills the wrapper wherever it had a size of its own.
#[cfg(target_os = "macos")]
fn move_placement(content: &mut StyleRefinement, wrapper: &mut StyleRefinement) {
    wrapper.position = Some(content.position.take().unwrap_or(gpui::Position::Relative));
    wrapper.inset = std::mem::take(&mut content.inset);
    wrapper.margin = std::mem::take(&mut content.margin);
    wrapper.flex_grow = content.flex_grow.take();
    wrapper.flex_shrink = content.flex_shrink.take();
    wrapper.flex_basis = content.flex_basis.take();
    wrapper.align_self = content.align_self.take();
    wrapper.size = std::mem::take(&mut content.size);
    wrapper.min_size = content.min_size.clone();
    wrapper.max_size = content.max_size.clone();
    let fill = Some(Length::Definite(gpui::relative(1.0).into()));
    if wrapper.size.width.is_some() || wrapper.flex_grow.is_some() {
        content.size.width = fill;
    }
    if wrapper.size.height.is_some() {
        content.size.height = fill;
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

/// Each window's native glass views, by element. They outlive the elements
/// that draw them: a closed panel's glass is only hidden, so opening it again
/// shows the same view rather than inserting a new one, which AppKit
/// materializes with a flash.
#[cfg(target_os = "macos")]
#[derive(Default)]
struct NativeGlassViews(
    std::collections::HashMap<
        (gpui::WindowId, gpui::ElementId),
        crate::platform::AppKitGlassBackdrop,
    >,
);

#[cfg(target_os = "macos")]
impl gpui::Global for NativeGlassViews {}

#[cfg(target_os = "macos")]
fn native_glass(
    id: &gpui::ElementId,
    window: &Window,
    cx: &mut App,
) -> Option<crate::platform::AppKitGlassBackdrop> {
    let key = (window.window_handle().window_id(), id.clone());
    if let Some(glass) = cx
        .try_global::<NativeGlassViews>()
        .and_then(|views| views.0.get(&key))
    {
        return Some(glass.clone());
    }
    let glass = crate::platform::AppKitGlassBackdrop::new()?;
    let open: Vec<_> = cx
        .windows()
        .iter()
        .map(|window| window.window_id())
        .collect();
    let views = &mut cx.default_global::<NativeGlassViews>().0;
    // Closed windows' glass goes with them.
    views.retain(|(window, _), _| open.contains(window));
    views.insert(key, glass.clone());
    Some(glass)
}
