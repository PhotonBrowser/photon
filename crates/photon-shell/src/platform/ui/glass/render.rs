//! Draws a [`Glass`]: frosted, over native glass, or as its plain fallback.

use gpui::{
    App, Div, ElementId, IntoElement, Length, Pixels, RenderOnce, Stateful, StyleRefinement,
    Styled, Window, div, prelude::*, px, rgba,
};

use super::super::metrics;
use super::super::motion::AnimateIn;
use super::super::settings::Settings;
use super::super::theme::{ThemeColors, with_opacity};
use super::{Glass, glass_available};

impl RenderOnce for Glass {
    fn render(self, window: &mut Window, cx: &mut App) -> impl IntoElement {
        let Glass {
            id,
            mut content,
            enabled,
            tint,
            backing,
            z_index,
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
        let backdrop = (enabled && frosted.is_none() && glass_available(cx))
            .then(|| native_glass(&id, window, cx))
            .flatten();
        #[cfg(not(target_os = "macos"))]
        let backdrop: Option<std::convert::Infallible> = {
            let _ = (enabled, &tint, backing, z_index, interactive, &window);
            None
        };

        let Some(backdrop) = backdrop else {
            let content = match fallback {
                Some(fallback) => fallback(content),
                None => content,
            };
            return animated(content, &id);
        };

        // The glass and the content are siblings, so the content's own
        // background and border draw over the glass. The outer wrapper takes
        // the content's place in its parent's layout; the inner one carries
        // the motion, as moving it moves the glass's frame with it.
        let mut placement = div();
        move_placement(content.style(), placement.style());
        let radius = corner_radius(content.style(), window);

        #[cfg(target_os = "macos")]
        let glass = {
            let glass = backdrop
                .corner_radius(radius)
                .window_tint(crate::platform::GlassWindowTint {
                    stops: if backing.is_some() { Vec::new() } else { tint },
                    angle_degrees: crate::platform::theme_config::theme()
                        .window
                        .gradient_angle_degrees,
                })
                .backing(backing)
                .z_index(z_index)
                .interactive(interactive)
                .absolute()
                .inset_0();
            // The glass moves with the inner wrapper and takes the scale
            // itself, by its frame. It does not fade, blur or take a layer
            // transform: native glass draws nothing while partly
            // transparent or transformed.
            match motion {
                Some((entrance, transition)) => glass.animate_with_offset_stable_id(
                    (id.clone(), "glass-motion"),
                    entrance.native_part(),
                    transition,
                    &motion_config,
                    (0.0, 0.0),
                ),
                None => glass.into_any_element(),
            }
        };
        #[cfg(not(target_os = "macos"))]
        let glass: gpui::AnyElement = match backdrop {};

        let moving = div()
            .id((id.clone(), "surface"))
            .relative()
            .size_full()
            .child(glass)
            .child(content);
        placement.child(animated(moving, &id)).into_any_element()
    }
}

/// The glass follows the element's top-left corner radius.
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
