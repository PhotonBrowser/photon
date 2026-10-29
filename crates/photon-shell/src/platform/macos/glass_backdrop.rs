//! Native Liquid Glass drawn beneath a GPUI element, in a window's glass
//! host beneath GPUI's view. `ui/glass.rs` builds on this; see
//! `Documentation/Glass.md`.

use std::{ptr::NonNull, rc::Rc, sync::atomic::Ordering};

use gpui::{
    App, Bounds, DispatchPhase, Element, ElementId, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, MouseButton, MouseDownEvent, MouseUpEvent, Pixels, Refineable, Size,
    Style, StyleRefinement, Styled, Visibility, Window, size,
};
use objc2::{
    MainThreadMarker, msg_send,
    rc::Retained,
    runtime::{AnyClass, AnyObject},
};
use objc2_app_kit::{
    NSApplication, NSGlassEffectView, NSGlassEffectViewStyle, NSView, NSWindowOrderingMode,
};
use objc2_core_graphics::CGColor;
use objc2_foundation::{NSArray, NSDictionary, NSNull, NSPoint, NSRect, NSSize, NSString};
use objc2_quartz_core::CAGradientLayer;

use super::appkit::{NEXT_NATIVE_VIEW_ID, appkit_host_view, enable_interactive_glass};
use super::page_layer;

/// Native interactive Liquid Glass drawn behind a GPUI element.
///
/// Unlike [`AppKitView`], the glass sits beneath GPUI's Metal view, in the
/// same sibling slot as the page layer, so the element's GPUI children draw
/// over it. The element clears GPUI's own painting over it, so the window
/// tint GPUI drew there is redrawn natively beneath the glass: the glass then
/// refracts the same tinted window as AppKit controls hosted above GPUI.
/// Left-button presses inside the element are forwarded so AppKit plays the
/// glass's own press response. Keep one in a retained view and render clones
/// of it as the first child of an absolutely sized parent.
#[derive(Clone)]
pub struct AppKitGlassBackdrop {
    inner: Rc<AppKitGlassBackdropInner>,
    corner_radius: Pixels,
    window_tint: GlassWindowTint,
    backing: Option<u32>,
    z_index: i32,
    interactive: bool,
    style: StyleRefinement,
}

/// The window tint GPUI paints, as evenly spaced gradient stops along a CSS
/// angle in degrees across the whole window.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct GlassWindowTint {
    /// RGBA stops; one stop is a solid tint.
    pub stops: Vec<u32>,
    pub angle_degrees: f32,
}

struct AppKitGlassBackdropInner {
    /// Clips the tint to the field and holds the glass above it.
    container: Retained<NSView>,
    tint: Retained<CAGradientLayer>,
    glass: Retained<NSGlassEffectView>,
    id: u64,
    /// The attachment that last placed the glass. GPUI can drop a stale
    /// attachment after a new one has shown the glass, as when the element's
    /// place in the tree changes; only the owner may hide it.
    owner: std::cell::Cell<u64>,
}

impl AppKitGlassBackdrop {
    /// Whether this Mac draws Liquid Glass, so callers can choose their
    /// painting before a backdrop exists.
    pub fn is_supported() -> bool {
        MainThreadMarker::new().is_some() && AnyClass::get(c"NSGlassEffectView").is_some()
    }

    /// Returns `None` before macOS 26, or off the AppKit main thread, so the
    /// caller keeps its own painted surface.
    pub fn new() -> Option<Self> {
        let mtm = MainThreadMarker::new()?;
        AnyClass::get(c"NSGlassEffectView")?;
        let container = NSView::new(mtm);
        container.setWantsLayer(true);
        let container_layer = container.layer()?;
        container_layer.setMasksToBounds(true);
        // Fade the glass as one picture; fading its layers separately lets
        // the edge highlights glow through as it goes.
        container_layer.setAllowsGroupOpacity(true);
        let tint = CAGradientLayer::new();
        // Without implicit animations the tint follows the field and window
        // in the same transaction as GPUI's frame.
        disable_layer_actions(&tint);
        container_layer.addSublayer(&tint);
        let glass = NSGlassEffectView::new(mtm);
        glass.setStyle(NSGlassEffectViewStyle::Regular);
        enable_interactive_glass(&glass);
        container.addSubview(&glass);
        // Forwarded presses end at the glass rather than travelling up the
        // responder chain, where the window could start a drag.
        // SAFETY: clearing the next responder is always valid; AppKit drops
        // unhandled events that reach the end of the chain.
        unsafe { glass.setNextResponder(None) };
        Some(Self {
            inner: Rc::new(AppKitGlassBackdropInner {
                container,
                tint,
                glass,
                id: NEXT_NATIVE_VIEW_ID.fetch_add(1, Ordering::Relaxed),
                owner: std::cell::Cell::new(0),
            }),
            corner_radius: Pixels::ZERO,
            window_tint: GlassWindowTint::default(),
            backing: None,
            z_index: 0,
            interactive: false,
            style: StyleRefinement::default(),
        })
    }

    /// The glass's corner radius, matching the GPUI surface it replaces.
    pub fn corner_radius(mut self, radius: Pixels) -> Self {
        self.corner_radius = radius;
        self
    }

    /// The window tint GPUI paints behind the element, which the glass
    /// refracts as AppKit controls over the window do.
    pub fn window_tint(mut self, tint: GlassWindowTint) -> Self {
        self.window_tint = tint;
        self
    }

    /// An RGBA surface beneath the tint, for glass resting on a GPUI surface
    /// over a page, such as the sidebar shown over it. The glass clears what
    /// GPUI painted, so without this the page would show through.
    pub fn backing(mut self, backing: Option<u32>) -> Self {
        self.backing = backing;
        self
    }

    /// Orders overlapping glass: higher draws over lower.
    pub fn z_index(mut self, z_index: i32) -> Self {
        self.z_index = z_index;
        self
    }

    /// Plays AppKit's press response when the element is pressed, for glass
    /// that is itself a control. Panels leave it off, so pressing their
    /// content does not move the glass beneath it.
    pub fn interactive(mut self, interactive: bool) -> Self {
        self.interactive = interactive;
        self
    }
}

impl Styled for AppKitGlassBackdrop {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl IntoElement for AppKitGlassBackdrop {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for AppKitGlassBackdrop {
    type RequestLayoutState = Style;
    /// The painted bounds, after the element's style scale, and where the
    /// glass fell in the order native views were placed.
    type PrepaintState = (Bounds<Pixels>, u64);

    fn id(&self) -> Option<ElementId> {
        Some(ElementId::named_usize(
            "appkit-glass-backdrop",
            self.inner.id as usize,
        ))
    }

    fn source_location(&self) -> Option<&'static std::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.refine(&self.style);
        (window.request_layout(style.clone(), [], cx), style)
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        style: &mut Self::RequestLayoutState,
        window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
        let global_id = id.expect("glass backdrop elements always have a stable id");
        let visible = style.visibility == Visibility::Visible;
        let alpha = style.opacity.unwrap_or(1.0).clamp(0.0, 1.0);
        // GPUI paints a style scale about the element's centre without moving
        // its layout, so the native glass follows that painted size.
        let scale = style.scale;
        let bounds = Bounds::centered_at(
            bounds.center(),
            size(
                bounds.size.width * scale.width,
                bounds.size.height * scale.height,
            ),
        );
        let inner = self.inner.clone();
        // Its corners stay as they are while it scales, so animating it
        // does not reshape the glass every frame.
        let radius = self.corner_radius;
        let tint = self.window_tint.clone();
        let backing = self.backing;
        let z_index = self.z_index;
        let viewport = window.viewport_size();
        window.with_element_state(global_id, |state: Option<GlassAttachment>, window| {
            let mut attachment = state.unwrap_or_else(|| GlassAttachment::new(inner));
            if let Some(host) = appkit_host_view(window) {
                // SAFETY: GPUI's live AppKit window owns this view throughout
                // this prepaint call.
                attachment.place(
                    unsafe { host.as_ref() },
                    GlassPlacement {
                        bounds,
                        viewport,
                        visible,
                        alpha,
                        radius,
                        tint: &tint,
                        backing,
                        z_index,
                    },
                );
            } else {
                attachment.detach();
            }
            ((), attachment)
        });
        (bounds, page_layer::next_placement_order())
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _style: &mut Self::RequestLayoutState,
        (bounds, placed): &mut Self::PrepaintState,
        window: &mut Window,
        _cx: &mut App,
    ) {
        // Every page has been placed by now, as GPUI prepaints the whole
        // frame before painting any of it.
        if let Some(host) = appkit_host_view(window) {
            // SAFETY: GPUI's live AppKit window owns this view while painting.
            let host = unsafe { host.as_ref() };
            let global_id = id.expect("glass backdrop elements always have a stable id");
            let inner = self.inner.clone();
            window.with_element_state(global_id, |state: Option<GlassAttachment>, _| {
                // Prepaint always leaves an attachment.
                let mut attachment = state.unwrap_or_else(|| GlassAttachment::new(inner));
                if let Some(beneath_pages) =
                    page_layer::page_placed_over(host, *bounds, attachment.placed, *placed)
                {
                    attachment.order(beneath_pages);
                }
                attachment.placed = *placed;
                ((), attachment)
            });
        }
        // Clear whatever GPUI painted here before, such as the window tint,
        // so the glass shows through as itself. The element's later children
        // still draw over it.
        if !is_collapsed(&self.inner.container) {
            let radius = self.corner_radius;
            window.paint_cutout(*bounds, radius.into(), 0.0);
        }
        if !self.interactive {
            return;
        }
        // The glass lies beneath GPUI's view and never receives AppKit hits,
        // so hand it the presses GPUI sees over it.
        let bounds = *bounds;
        let glass = self.inner.glass.clone();
        window.on_mouse_event(move |event: &MouseDownEvent, phase, _, _| {
            if phase == DispatchPhase::Capture
                && event.button == MouseButton::Left
                && bounds.contains(&event.position)
            {
                forward_current_event(&glass, |glass, event| glass.mouseDown(event));
            }
        });
        let glass = self.inner.glass.clone();
        window.on_mouse_event(move |event: &MouseUpEvent, phase, _, _| {
            if phase == DispatchPhase::Capture && event.button == MouseButton::Left {
                forward_current_event(&glass, |glass, event| glass.mouseUp(event));
            }
        });
    }
}

fn forward_current_event(
    glass: &NSGlassEffectView,
    forward: impl FnOnce(&NSGlassEffectView, &objc2_app_kit::NSEvent),
) {
    let Some(mtm) = MainThreadMarker::new() else {
        return;
    };
    if let Some(event) = NSApplication::sharedApplication(mtm).currentEvent() {
        forward(glass, &event);
    }
}

struct GlassAttachment {
    inner: Rc<AppKitGlassBackdropInner>,
    parent: Option<NonNull<NSView>>,
    /// What AppKit was last given, so unchanged frames touch nothing.
    applied: Option<AppliedGlass>,
    /// Where the glass was last painted in the placement order.
    placed: u64,
    beneath_pages: bool,
    z_index: i32,
    token: u64,
}

#[derive(PartialEq)]
struct AppliedGlass {
    bounds: Bounds<Pixels>,
    viewport: Size<Pixels>,
    host_bounds: NSRect,
    visible: bool,
    alpha: f32,
    radius: Pixels,
    tint: GlassWindowTint,
    backing: Option<u32>,
    z_index: i32,
}

struct GlassPlacement<'a> {
    bounds: Bounds<Pixels>,
    viewport: Size<Pixels>,
    visible: bool,
    alpha: f32,
    radius: Pixels,
    tint: &'a GlassWindowTint,
    backing: Option<u32>,
    z_index: i32,
}

impl GlassAttachment {
    fn new(inner: Rc<AppKitGlassBackdropInner>) -> Self {
        Self {
            inner,
            parent: None,
            applied: None,
            placed: 0,
            beneath_pages: false,
            z_index: 0,
            token: NEXT_NATIVE_VIEW_ID.fetch_add(1, Ordering::Relaxed),
        }
    }

    fn place(&mut self, host: &NSView, placement: GlassPlacement<'_>) {
        let GlassPlacement {
            bounds,
            viewport,
            visible,
            alpha,
            radius,
            tint,
            backing,
            z_index,
        } = placement;
        self.inner.owner.set(self.token);
        // SAFETY: AppKit view hierarchy access runs on the main thread.
        let Some(window_content) = (unsafe { host.superview() }) else {
            self.detach();
            return;
        };
        let applied = AppliedGlass {
            bounds,
            viewport,
            host_bounds: host.bounds(),
            visible,
            alpha,
            radius,
            tint: tint.clone(),
            backing,
            z_index,
        };
        // Unchanged frames touch nothing, not even the host lookup.
        if self.parent.is_some() && self.applied.as_ref() == Some(&applied) {
            return;
        }
        // Glass lies directly in the window's content, where it draws while
        // it animates, beside the pages' host and beneath GPUI's view.
        let parent = window_content;
        let container = &self.inner.container;
        let gradient_changed = self
            .applied
            .as_ref()
            .is_none_or(|last| last.tint != applied.tint || last.viewport != viewport);
        let moved = self
            .applied
            .as_ref()
            .is_none_or(|last| last.bounds != bounds);
        let backing_changed = self
            .applied
            .as_ref()
            .is_none_or(|last| last.backing != backing);
        let radius_changed = self
            .applied
            .as_ref()
            .is_none_or(|last| last.radius != radius);
        self.applied = Some(applied);
        // SAFETY: as above.
        let current_parent = unsafe { container.superview() };
        if !current_parent
            .as_ref()
            .is_some_and(|current| std::ptr::eq(&**current, &*parent))
        {
            // Inserted once: adding, hiding or removing a view here redraws
            // the whole window's glass, so glass is never hidden or removed
            // again while its window lives, and orders by depth alone.
            container.removeFromSuperview();
            parent.addSubview_positioned_relativeTo(
                container,
                NSWindowOrderingMode::Below,
                Some(host),
            );
        }
        self.parent = Some(NonNull::from(&*parent));

        let host_bounds = host.bounds();
        let y = if host.isFlipped() {
            host_bounds.origin.y + bounds.origin.y.as_f32() as f64
        } else {
            host_bounds.origin.y + host_bounds.size.height
                - bounds.origin.y.as_f32() as f64
                - bounds.size.height.as_f32() as f64
        };
        let frame = NSRect::new(
            NSPoint::new(host_bounds.origin.x + bounds.origin.x.as_f32() as f64, y),
            NSSize::new(
                bounds.size.width.as_f32() as f64,
                bounds.size.height.as_f32() as f64,
            ),
        );
        // Glass is never hidden: showing a hidden glass view plays AppKit's
        // appearance, which draws nothing at first. Glass that should not
        // show moves out of sight instead.
        let shown =
            visible && bounds.size.width > Pixels::ZERO && bounds.size.height > Pixels::ZERO;
        if shown {
            container.setFrame(host.convertRect_toView(frame, Some(&parent)));
        } else {
            collapse(container);
        }
        if container.isHidden() {
            container.setHidden(false);
        }
        container.setAlphaValue(alpha as f64);
        let glass = &self.inner.glass;
        glass.setFrame(container.bounds());
        if radius_changed {
            if let Some(layer) = container.layer() {
                layer.setCornerRadius(radius.as_f32() as f64);
            }
            glass.setCornerRadius(radius.as_f32() as f64);
        }
        self.z_index = z_index;
        self.apply_depth();
        if backing_changed && let Some(layer) = container.layer() {
            let backing = backing.map(cg_color);
            layer.setBackgroundColor(backing.as_deref());
        }
        if moved || gradient_changed {
            self.place_tint(bounds, viewport, container.isFlipped());
        }
        if gradient_changed {
            self.paint_tint(viewport, container.isFlipped(), tint);
        }
    }

    /// Keeps the glass in GPUI's paint order among the pages: in the host
    /// beneath them when one drawn after the glass covers it, as a peek covers
    /// the chrome, and in the host above them otherwise, as the chrome over a
    /// page or a popover is. Moving glass makes AppKit materialize it afresh,
    /// so this happens only when that order changes.
    fn order(&mut self, beneath_pages: bool) {
        self.beneath_pages = beneath_pages;
        self.apply_depth();
    }

    /// Sets the glass's layer depth from its place among the pages and its
    /// z-index. Depth orders it without moving it among the views.
    fn apply_depth(&self) {
        let Some(layer) = self.inner.container.layer() else {
            return;
        };
        let depth = page_layer::glass_depth(self.beneath_pages, self.z_index);
        if layer.zPosition() != depth {
            layer.setZPosition(depth);
        }
    }

    /// Lays the window-sized tint gradient under the field so the slice the
    /// field covers matches what GPUI paints around it.
    fn place_tint(&self, bounds: Bounds<Pixels>, viewport: Size<Pixels>, flipped: bool) {
        let layer = &self.inner.tint;
        let (width, height) = (
            viewport.width.as_f32() as f64,
            viewport.height.as_f32() as f64,
        );
        let (left, top) = (
            bounds.origin.x.as_f32() as f64,
            bounds.origin.y.as_f32() as f64,
        );
        let bottom = height - top - bounds.size.height.as_f32() as f64;
        layer.setFrame(NSRect::new(
            NSPoint::new(-left, if flipped { -top } else { -bottom }),
            NSSize::new(width, height),
        ));
    }

    /// Colours the window-sized tint gradient, only when its colours or the
    /// window change.
    fn paint_tint(&self, viewport: Size<Pixels>, flipped: bool, tint: &GlassWindowTint) {
        let layer = &self.inner.tint;
        let (width, height) = (
            viewport.width.as_f32() as f64,
            viewport.height.as_f32() as f64,
        );
        let colors: Vec<_> = match tint.stops.as_slice() {
            [] => vec![cg_color(0), cg_color(0)],
            [solid] => vec![cg_color(*solid), cg_color(*solid)],
            stops => stops.iter().copied().map(cg_color).collect(),
        };
        let colors: Vec<&AnyObject> = colors
            .iter()
            .map(|color| {
                let color: &CGColor = color;
                // SAFETY: CGColor is a toll-free Core Foundation object.
                unsafe { &*(color as *const CGColor).cast::<AnyObject>() }
            })
            .collect();
        let colors = NSArray::from_slice(&colors);
        // SAFETY: the array holds only CGColor objects, as `colors` requires.
        unsafe { layer.setColors(Some(&colors)) };

        // CSS gradient line, in unit coordinates with y pointing down.
        let radians = (tint.angle_degrees as f64).to_radians();
        let (dx, dy) = (radians.sin(), -radians.cos());
        let half = (dx.abs() * width + dy.abs() * height) / 2.0;
        let unit = |sign: f64| {
            let x = 0.5 + sign * dx * half / width.max(1.0);
            let y = 0.5 + sign * dy * half / height.max(1.0);
            // Core Animation's unit space follows the layer's geometry.
            NSPoint::new(x, if flipped { y } else { 1.0 - y })
        };
        layer.setStartPoint(unit(-1.0));
        layer.setEndPoint(unit(1.0));
    }

    fn detach(&mut self) {
        self.applied = None;
        let Some(parent) = self.parent.take() else {
            return;
        };
        // SAFETY: AppKit view hierarchy access stays on the main thread.
        let current_parent = unsafe { self.inner.container.superview() };
        if current_parent
            .as_ref()
            .is_some_and(|current| std::ptr::eq(&**current, parent.as_ptr()))
        {
            self.inner.container.removeFromSuperview();
        }
    }
}

fn cg_color(rgba: u32) -> impl std::ops::Deref<Target = CGColor> {
    let [red, green, blue, alpha] = rgba.to_be_bytes().map(|channel| f64::from(channel) / 255.0);
    CGColor::new_srgb(red, green, blue, alpha)
}

fn disable_layer_actions(layer: &CAGradientLayer) {
    let keys = [
        "bounds",
        "position",
        "colors",
        "startPoint",
        "endPoint",
        "hidden",
    ]
    .map(NSString::from_str);
    let null = NSNull::null();
    let actions = NSDictionary::from_slices(
        &keys.iter().map(|key| &**key).collect::<Vec<_>>(),
        &vec![&*null; keys.len()],
    );
    // SAFETY: NSNull is a valid CAAction that disables an animation.
    let _: () = unsafe { msg_send![layer, setActions: &*actions] };
}

impl Drop for GlassAttachment {
    /// A frame that does not draw the element drops its attachment. Move the
    /// glass out of sight rather than remove or hide it: inserting or showing
    /// a glass view again makes AppKit materialize it afresh, which draws
    /// nothing at first, as when a panel or the sidebar comes back.
    fn drop(&mut self) {
        if self.parent.is_some() && self.inner.owner.get() == self.token {
            collapse(&self.inner.container);
        }
    }
}

/// Where collapsed glass waits, far outside any window.
const COLLAPSED_ORIGIN: f64 = -100_000.0;

/// Moves glass out of sight, keeping its size and shape, in place of hiding
/// it: shown again, it only moves back, which glass draws at once.
fn collapse(container: &NSView) {
    let size = container.frame().size;
    container.setFrame(NSRect::new(
        NSPoint::new(COLLAPSED_ORIGIN, COLLAPSED_ORIGIN),
        size,
    ));
}

fn is_collapsed(container: &NSView) -> bool {
    let frame = container.frame();
    frame.origin.x == COLLAPSED_ORIGIN || frame.size.width == 0.0 || frame.size.height == 0.0
}

impl Drop for AppKitGlassBackdropInner {
    fn drop(&mut self) {
        self.container.removeFromSuperview();
    }
}
