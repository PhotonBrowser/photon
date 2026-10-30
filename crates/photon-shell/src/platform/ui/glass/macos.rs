//! macOS native glass: AppKit Liquid Glass views placed beneath GPUI content.

use gpui::{App, Div, Length, Pixels, Stateful, StyleRefinement, Window, div, prelude::*};

/// How native glass lies beneath a [`Glass`]'s content, on macOS.
pub(super) struct NativeGlass {
    pub(super) tint: Vec<u32>,
    pub(super) backing: Option<u32>,
    pub(super) interactive: bool,
}

impl NativeGlass {
    /// Draws `content`, animated by `animate`, over `backdrop`. The glass
    /// and the content are siblings, so the content's own background and
    /// border draw over the glass; a wrapper takes the content's place in its
    /// parent's layout. The glass stays still: it draws nothing while it
    /// changes.
    pub(super) fn under(
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

/// Each window's native glass views, by element. They outlive the elements
/// that draw them: a closed panel's glass is only hidden, so opening it again
/// shows the same view rather than inserting a new one, which AppKit
/// materializes with a flash.
#[derive(Default)]
struct NativeGlassViews(
    std::collections::HashMap<
        (gpui::WindowId, gpui::ElementId),
        crate::platform::AppKitGlassBackdrop,
    >,
);

impl gpui::Global for NativeGlassViews {}

pub(super) fn native_glass(
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
