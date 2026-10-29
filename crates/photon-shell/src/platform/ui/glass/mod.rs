//! Liquid Glass behind any GPUI element. See `Documentation/Glass.md`.
//!
//! This module is the builder; `render` turns it into native or frosted
//! glass, or the plain fallback.
//!
//! [`glass`] builds a `div` with native glass beneath its children on macOS
//! 26+. Off macOS, before macOS 26, or with the "Glass menus" setting off, it
//! is that `div` alone, with any [`Glass::fallback`] styles applied.

mod render;

use gpui::{
    AnyElement, App, Div, ElementId, Interactivity, IntoElement, ParentElement, Stateful,
    StatefulInteractiveElement, StyleRefinement, Styled, div, prelude::*,
};

use super::motion::{Entrance, Transition};
use super::settings::Settings;
use super::theme::ThemeColors;

/// Glass `z_index` layers, lowest first. Overlapping glass draws in this
/// order whatever order GPUI drew it in.
pub(super) mod layer {
    /// Glass resting on the window, such as the address field.
    pub(in super::super) const CHROME: i32 = 0;
    /// Panels over the chrome, such as the theme picker.
    pub(in super::super) const POPOVER: i32 = 1;
}

/// Whether glass draws: the platform has it and the settings allow it.
pub(super) fn glass_available(cx: &App) -> bool {
    #[cfg(target_os = "macos")]
    return Settings::get(cx).glass_surfaces
        && crate::platform::AppKitGlassBackdrop::is_supported();
    #[cfg(not(target_os = "macos"))]
    {
        let _ = cx;
        false
    }
}

/// A `div` drawn over native glass. Style and fill it as any `div`: its
/// corner radius shapes the glass, and its background and border draw on it.
pub(super) fn glass(id: impl Into<ElementId>) -> Glass {
    let id = id.into();
    Glass {
        content: div().id(id.clone()),
        id,
        enabled: true,
        tint: Vec::new(),
        backing: None,
        z_index: 0,
        interactive: false,
        frosted: None,
        motion: None,
        fallback: None,
    }
}

type Fallback = Box<dyn FnOnce(Stateful<Div>) -> Stateful<Div>>;

#[derive(IntoElement)]
pub(super) struct Glass {
    id: ElementId,
    content: Stateful<Div>,
    enabled: bool,
    tint: Vec<u32>,
    backing: Option<u32>,
    z_index: i32,
    interactive: bool,
    /// Frosted glass's palette; `None` for native glass.
    frosted: Option<ThemeColors>,
    motion: Option<(Entrance, Transition)>,
    fallback: Option<Fallback>,
}

impl Glass {
    /// Turns the glass on or off for this element; it is on by default.
    /// Off, the element has its fallback styles.
    pub(super) fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Styles applied only where there is no glass, such as a painted surface.
    pub(super) fn fallback(
        mut self,
        styles: impl FnOnce(Stateful<Div>) -> Stateful<Div> + 'static,
    ) -> Self {
        self.fallback = Some(Box::new(styles));
        self
    }

    /// Lays the window's tint beneath the glass, for glass resting on the
    /// window itself, so it refracts the colour GPUI paints around it.
    pub(super) fn window_tint(mut self, palette: ThemeColors) -> Self {
        self.tint = [
            Some(palette.window_tint),
            palette.window_tint_middle,
            Some(palette.window_tint_end),
        ]
        .into_iter()
        .flatten()
        .collect();
        self
    }

    /// An RGBA surface beneath the glass, for glass resting on a GPUI surface
    /// over a page. Glass shows what lies beneath GPUI, so without this the
    /// page shows through. It replaces the window tint.
    pub(super) fn backing(mut self, backing: Option<u32>) -> Self {
        self.backing = backing;
        self
    }

    /// Orders overlapping glass: higher draws over lower. All glass lies
    /// beneath GPUI's own drawing.
    pub(super) fn z_index(mut self, z_index: i32) -> Self {
        self.z_index = z_index;
        self
    }

    /// Plays AppKit's press response when the element is pressed, for glass
    /// that is itself a control, such as the address field. Panels leave it
    /// off, so pressing their content leaves the glass still.
    pub(super) fn interactive(mut self, interactive: bool) -> Self {
        self.interactive = interactive;
        self
    }

    /// Draws GPUI's own frosted glass instead of native glass: a blur of
    /// whatever lies behind, GPUI's drawing included, under a translucent
    /// `palette` surface. Use it for glass over GPUI content, such as a panel
    /// on the new tab page, which native glass cannot show. It works on every
    /// platform and animates as any GPUI element.
    pub(super) fn frosted(mut self, palette: ThemeColors) -> Self {
        self.frosted = Some(palette);
        self
    }

    /// Animates the element with `entrance`. Native glass does not follow
    /// GPUI's painted transforms, so it takes the same motion itself.
    pub(super) fn motion(mut self, entrance: Entrance, transition: Transition) -> Self {
        self.motion = Some((entrance, transition));
        self
    }
}

impl Styled for Glass {
    fn style(&mut self) -> &mut StyleRefinement {
        self.content.style()
    }
}

impl ParentElement for Glass {
    fn extend(&mut self, elements: impl IntoIterator<Item = AnyElement>) {
        self.content.extend(elements);
    }
}

impl InteractiveElement for Glass {
    fn interactivity(&mut self) -> &mut Interactivity {
        self.content.interactivity()
    }
}

impl StatefulInteractiveElement for Glass {}
