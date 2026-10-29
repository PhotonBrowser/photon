//! The Core Animation layer that shows a page's Engine IOSurface.
//!
//! The layer sits beneath GPUI's layer, which cuts a hole where the page is
//! (see `Window::paint_cutout`). Core Animation and the window server
//! composite the page without copying it through GPUI, and anything GPUI
//! draws over the page, such as a tooltip or menu, stays above it. This
//! follows Ladybird's Qt view, which shows its IOSurface in a layer of its
//! own native view.
#![allow(deprecated)]

use std::cell::{Cell, RefCell};

use gpui::{Bounds, Pixels, Window};
use io_surface::IOSurface;
use objc2::{MainThreadMarker, msg_send, rc::Retained, runtime::AnyObject};
use objc2_app_kit::{NSView, NSWindowOrderingMode};
use objc2_core_graphics::CGColor;
use objc2_foundation::{NSDictionary, NSNull, NSPoint, NSRect, NSSize, NSString};
use objc2_quartz_core::{
    CALayer, kCACornerCurveContinuous, kCAGravityBottomLeft, kCAGravityResize, kCAGravityTopLeft,
};

use super::appkit::appkit_host_view;
use crate::platform::PagePlacement;

/// How much more the layer is rounded than GPUI's cutout, in points.
const CORNER_INSET: f32 = 1.5;

/// Identifies a frame by Engine backing, generation and frame number.
pub(super) type FrameIdentity = (u64, u64, u64);

pub(super) struct PageLayer {
    layer: Retained<CALayer>,
    /// Retained until a later transaction replaces the layer's contents.
    current_surface: RefCell<Option<IOSurface>>,
    current_identity: Cell<Option<FrameIdentity>>,
    contents_fraction: Cell<(f64, f64)>,
    /// The colour at the painted frame's edge, as 0xRRGGBB, which fills
    /// uncovered area while Engine lays out at a new size.
    edge_color: Cell<Option<u32>>,
    /// What `place` last applied, so an unchanged layout touches nothing.
    placed: Cell<Option<(PagePlacement, NSRect, bool)>>,
}

impl PageLayer {
    pub(super) fn new() -> Self {
        let _main_thread = MainThreadMarker::new()
            .expect("Core Animation layers must be created on the AppKit main thread");
        let layer = CALayer::layer();
        layer.setMasksToBounds(true);
        // Without animations, changes join the main thread's implicit
        // transaction: the one GPUI-CE presents a frame whose cutout moved
        // with, so the page and its hole reach the screen together.
        let keys = [
            "bounds",
            "position",
            "hidden",
            "opacity",
            "contents",
            "contentsRect",
            "contentsScale",
            "contentsGravity",
            "backgroundColor",
            "cornerRadius",
            "onOrderIn",
            "onOrderOut",
        ]
        .map(NSString::from_str);
        let null = NSNull::null();
        let actions = NSDictionary::from_slices(
            &keys.iter().map(|key| &**key).collect::<Vec<_>>(),
            &vec![&*null; keys.len()],
        );
        // SAFETY: NSNull is a valid CAAction that disables an animation.
        let _: () = unsafe { msg_send![&*layer, setActions: &*actions] };
        // SAFETY: Core Animation exposes these process-wide immutable values.
        unsafe {
            layer.setCornerCurve(kCACornerCurveContinuous);
        }
        Self {
            layer,
            current_surface: RefCell::new(None),
            current_identity: Cell::new(None),
            contents_fraction: Cell::new((1.0, 1.0)),
            edge_color: Cell::new(None),
            placed: Cell::new(None),
        }
    }

    /// Whether the layer already shows this frame.
    pub(super) fn shows(&self, identity: FrameIdentity) -> bool {
        self.current_identity.get() == Some(identity)
    }

    /// Shows a frame, without painting page pixels through GPUI.
    pub(super) fn set_surface(
        &self,
        identity: FrameIdentity,
        surface: &IOSurface,
        painted_width: i32,
        painted_height: i32,
    ) {
        if self.shows(identity) {
            return;
        }
        let width = unsafe { io_surface::IOSurfaceGetWidth(surface.obj) } as f64;
        let height = unsafe { io_surface::IOSurfaceGetHeight(surface.obj) } as f64;
        if painted_width <= 0 || painted_height <= 0 || width <= 0.0 || height <= 0.0 {
            self.clear_surface();
            return;
        }

        // A resize can pad the surface beyond what was painted; show only
        // the painted part.
        let contents_fraction = (
            f64::from(painted_width).min(width) / width,
            f64::from(painted_height).min(height) / height,
        );
        self.contents_fraction.set(contents_fraction);
        self.layer
            .setContentsRect(contents_rect(&self.layer, contents_fraction));
        // IOSurface is a Core Foundation object accepted by CALayer.contents.
        let contents = unsafe { &*surface.obj.cast::<AnyObject>() };
        unsafe { self.layer.setContents(Some(contents)) };
        // Engine's reported page background can differ from what the page
        // shows at its edge, and flashes through as the window grows.
        let edge_color = edge_color(surface, painted_width, painted_height);
        if edge_color.is_some() && edge_color != self.edge_color.get() {
            self.edge_color.set(edge_color);
            if let Some((placement, _, _)) = self.placed.get() {
                set_background_color(&self.layer, edge_color.unwrap_or(placement.background));
            }
        }
        // The main thread's implicit transaction commits this. Ladybird's Qt
        // view also asks for a window display cycle, to commit with Qt's own
        // backing store and keep Qt routing mouse events to its native view;
        // AppKit routes events to GPUI's view by view, not by layer.
        *self.current_surface.borrow_mut() = Some(surface.clone());
        self.current_identity.set(Some(identity));
    }

    fn clear_surface(&self) {
        if self.current_identity.get().is_none() {
            return;
        }
        unsafe { self.layer.setContents(None) };
        *self.current_surface.borrow_mut() = None;
        self.current_identity.set(None);
        self.contents_fraction.set((1.0, 1.0));
        self.edge_color.set(None);
    }

    /// Positions the layer beneath the page's hole in GPUI's layer. Runs in
    /// every GPUI prepaint of the page, so it changes the layer only when
    /// the page's layout did.
    pub(super) fn place(&self, window: &Window, placement: PagePlacement) {
        let Some(host) = appkit_host_view(window) else {
            self.detach();
            return;
        };
        // SAFETY: GPUI owns its AppKit window view throughout prepaint.
        let host = unsafe { host.as_ref() };
        let entry = (
            &*self.layer as *const CALayer as usize,
            host as *const NSView as usize,
            placement.bounds,
            next_placement_order(),
        );
        PAGE_PLACEMENTS.with_borrow_mut(|pages| {
            pages.retain(|&(key, ..)| key != entry.0);
            pages.push(entry);
        });
        let Some(host_layer) = host.layer() else {
            self.layer.setHidden(true);
            self.placed.set(None);
            return;
        };

        // Pages live in a layer-hosting view of Photon's own beneath GPUI's,
        // never in a layer AppKit manages: with a Liquid Glass window the
        // parent is the glass's content, and changing its sublayers, as when a
        // page hides or a preview attaches, makes the glass redraw the whole
        // window. If GPUI's view has no parent, the page sits behind its
        // contents instead.
        // SAFETY: AppKit view hierarchy access runs on the main thread.
        let page_host = unsafe { host.superview() }.map(|parent| page_host_view(host, &parent));
        let has_sibling_layer = page_host.is_some();
        let parent_layer = match page_host.as_ref().and_then(|view| view.layer()) {
            Some(layer) => layer,
            None => host_layer.clone(),
        };
        let layer = &self.layer;
        let attached = layer
            .superlayer()
            .is_some_and(|parent| std::ptr::eq(&*parent, &*parent_layer));

        // Convert through AppKit's views, which honour each view's flipping,
        // as the glass backdrop does; a layer's backing view shares its
        // coordinate space. Converting between the layers mirrors the page
        // vertically once GPUI's view has a layer-backed superview.
        let bounds = placement.bounds;
        let host_bounds = host.bounds();
        let (width, height) = (
            f64::from(bounds.size.width.as_f32()),
            f64::from(bounds.size.height.as_f32()),
        );
        let top = f64::from(bounds.origin.y.as_f32());
        let y = if host.isFlipped() {
            host_bounds.origin.y + top
        } else {
            host_bounds.origin.y + host_bounds.size.height - top - height
        };
        let host_page_bounds = NSRect::new(
            NSPoint::new(
                host_bounds.origin.x + f64::from(bounds.origin.x.as_f32()),
                y,
            ),
            NSSize::new(width, height),
        );
        let frame = snap_to_pixels(
            match &page_host {
                Some(page_host) => host.convertRect_toView(host_page_bounds, Some(page_host)),
                None => host_page_bounds,
            },
            f64::from(placement.scale_factor),
        );
        let flipped = parent_layer.isGeometryFlipped();
        if attached && !layer.isHidden() && self.placed.get() == Some((placement, frame, flipped)) {
            return;
        }

        if !attached {
            layer.removeFromSuperlayer();
            if has_sibling_layer {
                // A page attached later, like a link preview over its page,
                // lies above those attached before.
                parent_layer.addSublayer(layer);
                layer.setZPosition(0.0);
            } else {
                // A root layer has no sibling slot. Keep the page behind the
                // GPUI contents so the painted cutout exposes it.
                parent_layer.addSublayer(layer);
                layer.setZPosition(-1.0);
            }
        }
        layer.setGeometryFlipped(flipped);
        set_contents_gravity(layer, placement.scaled);
        layer.setContentsRect(contents_rect(layer, self.contents_fraction.get()));
        layer.setFrame(frame);
        // Core Animation's continuous corner and GPUI's smoothed cutout differ
        // slightly, and where the layer reached past the hole its page showed
        // through the translucent window as a light hairline. Round the layer
        // a little more, so its corner always lies inside the hole.
        let corner_radius = if placement.corner_radius > 0.0 {
            placement.corner_radius + CORNER_INSET
        } else {
            0.0
        };
        layer.setCornerRadius(f64::from(corner_radius));
        layer.setOpacity(placement.opacity.clamp(0.0, 1.0));
        layer.setContentsScale(f64::from(placement.scale_factor));
        // Shown until the first frame arrives, as the page's own background,
        // then as the colour at the frame's edge.
        let background = self.edge_color.get().unwrap_or(placement.background);
        set_background_color(layer, background);
        layer.setHidden(false);
        self.placed.set(Some((placement, frame, flipped)));
    }

    /// Hides the layer while its page is not shown, as for a background tab.
    /// A hidden page gets no GPUI prepaint, so `place` cannot hide it, and its
    /// last frame would show through the next page's hole. The next `place`
    /// shows it again.
    pub(super) fn hide(&self) {
        forget_placement(&self.layer);
        self.layer.setHidden(true);
    }

    /// Removes the layer from its window, as when the page closes.
    pub(super) fn detach(&self) {
        forget_placement(&self.layer);
        self.clear_surface();
        self.layer.removeFromSuperlayer();
        self.placed.set(None);
    }
}

impl Drop for PageLayer {
    fn drop(&mut self) {
        self.layer.removeFromSuperlayer();
    }
}

/// A page layer, its GPUI view, its bounds and its place in the order.
type PagePlaced = (usize, usize, Bounds<Pixels>, u64);

thread_local! {
    /// Counts native placements in GPUI's prepaint order, which is its paint
    /// order, so glass can tell which pages were drawn after it.
    static PLACEMENT_ORDER: Cell<u64> = const { Cell::new(0) };
    /// Where each shown page was last placed, by layer: its GPUI view, its
    /// bounds and its place in the placement order.
    static PAGE_PLACEMENTS: RefCell<Vec<PagePlaced>> =
        const { RefCell::new(Vec::new()) };
}

/// The next number in the order native views are placed in.
pub(super) fn next_placement_order() -> u64 {
    PLACEMENT_ORDER.with(|order| {
        let next = order.get() + 1;
        order.set(next);
        next
    })
}

/// Whether a page overlapping `bounds` in the GPUI view `host` was drawn
/// over something placed at `placed`, as a peek is over chrome: `Some(true)`
/// when one was placed after it, `Some(false)` when they were all placed
/// after `previous` and before it, or none overlaps. Pages not placed since
/// `previous`, as in a frame GPUI reused, decide nothing.
pub(super) fn page_placed_over(
    host: &NSView,
    bounds: Bounds<Pixels>,
    previous: u64,
    placed: u64,
) -> Option<bool> {
    let host = host as *const NSView as usize;
    PAGE_PLACEMENTS.with_borrow(|pages| {
        let mut decided = Some(false);
        let mut fresh = false;
        for &(_, page_host, page_bounds, order) in pages.iter() {
            if page_host != host || !page_bounds.intersects(&bounds) {
                continue;
            }
            if order > placed {
                return Some(true);
            }
            if order > previous {
                fresh = true;
            } else if !fresh {
                decided = None;
            }
        }
        if fresh { Some(false) } else { decided }
    })
}

fn forget_placement(layer: &CALayer) {
    let layer = layer as *const CALayer as usize;
    PAGE_PLACEMENTS.with_borrow_mut(|pages| pages.retain(|&(key, ..)| key != layer));
}

thread_local! {
    /// Each window's page host, by the GPUI view it lies beneath.
    static PAGE_HOSTS: RefCell<Vec<(usize, Retained<NSView>)>> = const { RefCell::new(Vec::new()) };
}

/// The page host beneath GPUI's view `host`, once a page has made one.
pub(super) fn page_host_for(host: &NSView) -> Option<Retained<NSView>> {
    let key = std::ptr::from_ref(host) as usize;
    PAGE_HOSTS.with_borrow(|hosts| {
        hosts
            .iter()
            .find(|(host_key, _)| *host_key == key)
            .map(|(_, view)| view.clone())
    })
}

/// The layer-hosting view that holds a window's pages, directly beneath
/// GPUI's view `host` in `parent`, created the first time it is needed.
fn page_host_view(host: &NSView, parent: &NSView) -> Retained<NSView> {
    let key = std::ptr::from_ref(host) as usize;
    PAGE_HOSTS.with_borrow_mut(|hosts| {
        // A closed window's host goes with it; its address may be reused.
        hosts.retain(|(_, view)| unsafe { view.superview() }.is_some());
        let existing = hosts.iter().find(|(host_key, view)| {
            *host_key == key
                && unsafe { view.superview() }
                    .is_some_and(|current| std::ptr::eq(&*current, parent))
        });
        let view = match existing {
            Some((_, view)) => view.clone(),
            None => {
                let mtm = MainThreadMarker::new()
                    .expect("page hosts are created on the AppKit main thread");
                let view = NSView::initWithFrame(mtm.alloc(), host.frame());
                // A layer-hosting view: AppKit leaves its layer tree to us.
                view.setLayer(Some(&CALayer::layer()));
                view.setWantsLayer(true);
                parent.addSubview_positioned_relativeTo(
                    &view,
                    NSWindowOrderingMode::Below,
                    Some(host),
                );
                hosts.push((key, view.clone()));
                view
            }
        };
        if view.frame() != host.frame() {
            view.setFrame(host.frame());
        }
        view
    })
}

/// Aligns the frame's edges to device pixels, as GPUI aligns the cutout's,
/// so no sliver of what lies behind shows along the page's edges.
fn snap_to_pixels(frame: NSRect, scale: f64) -> NSRect {
    if scale <= 0.0 {
        return frame;
    }
    let snap = |value: f64| (value * scale).round() / scale;
    let (left, top) = (snap(frame.origin.x), snap(frame.origin.y));
    let right = snap(frame.origin.x + frame.size.width);
    let bottom = snap(frame.origin.y + frame.size.height);
    NSRect::new(
        NSPoint::new(left, top),
        NSSize::new(right - left, bottom - top),
    )
}

/// Pins frames to the page's visual top-left at their own size, as WebKit
/// does. While Engine lays out at a new size, the last frame stays sharp and
/// unscaled, and the layer's background, the page's own, fills the rest.
/// A `scaled` page is stretched to the layer instead.
fn set_contents_gravity(layer: &CALayer, scaled: bool) {
    if scaled {
        // SAFETY: Core Animation exposes these process-wide immutable values.
        layer.setContentsGravity(unsafe { kCAGravityResize });
        return;
    }
    // Gravity's top is the layer's maximum y unless its contents are flipped,
    // as in `contents_rect`.
    // SAFETY: Core Animation exposes these process-wide immutable values.
    let gravity = unsafe {
        if layer.contentsAreFlipped() {
            kCAGravityBottomLeft
        } else {
            kCAGravityTopLeft
        }
    };
    layer.setContentsGravity(gravity);
}

/// The painted part of the surface, measured from its visual top-left.
fn contents_rect(layer: &CALayer, (width, height): (f64, f64)) -> NSRect {
    let y = if layer.contentsAreFlipped() {
        0.0
    } else {
        1.0 - height
    };
    NSRect::new(NSPoint::new(0.0, y), NSSize::new(width, height))
}

fn set_background_color(layer: &CALayer, rgb: u32) {
    let [_, red, green, blue] = rgb.to_be_bytes();
    let color = CGColor::new_srgb(
        f64::from(red) / 255.0,
        f64::from(green) / 255.0,
        f64::from(blue) / 255.0,
        1.0,
    );
    layer.setBackgroundColor(Some(&color));
}

/// Estimates the colour at the frame's painted right edge, as 0xRRGGBB.
/// Engine paints BGRA surfaces; other layouts are left to the page background.
fn edge_color(surface: &IOSurface, painted_width: i32, painted_height: i32) -> Option<u32> {
    const READ_ONLY: u32 = 1;
    const SAMPLE_RADIUS: usize = 4;
    const SAMPLE_COUNT: usize = (SAMPLE_RADIUS * 2 + 1).pow(2);
    let obj = surface.obj;
    // SAFETY: the surface is retained by `surface`, and is locked read-only
    // around the read, which stays within its painted bounds. Taking the
    // median of a small patch prevents one glyph or antialiased pixel from
    // colouring the whole area exposed during a resize.
    unsafe {
        if io_surface::IOSurfaceGetBytesPerElement(obj) != 4 {
            return None;
        }
        let painted_width = usize::try_from(painted_width).ok()?;
        let painted_height = usize::try_from(painted_height).ok()?;
        if painted_width == 0 || painted_height == 0 {
            return None;
        }
        let surface_width = io_surface::IOSurfaceGetWidth(obj);
        let surface_height = io_surface::IOSurfaceGetHeight(obj);
        if painted_width > surface_width || painted_height > surface_height {
            return None;
        }
        if io_surface::IOSurfaceLock(obj, READ_ONLY, std::ptr::null_mut()) != 0 {
            return None;
        }
        let base = io_surface::IOSurfaceGetBaseAddress(obj).cast::<u8>();
        if base.is_null() {
            io_surface::IOSurfaceUnlock(obj, READ_ONLY, std::ptr::null_mut());
            return None;
        }
        let row = io_surface::IOSurfaceGetBytesPerRow(obj);
        let x_start = painted_width.saturating_sub(SAMPLE_RADIUS + 1);
        let x_end = painted_width;
        let center_y = painted_height / 2;
        let y_start = center_y.saturating_sub(SAMPLE_RADIUS);
        let y_end = (center_y + SAMPLE_RADIUS + 1).min(painted_height);
        let mut red = [0; SAMPLE_COUNT];
        let mut green = [0; SAMPLE_COUNT];
        let mut blue = [0; SAMPLE_COUNT];
        let mut samples = 0;
        for y in y_start..y_end {
            for x in x_start..x_end {
                let pixel = std::slice::from_raw_parts(base.add(y * row + x * 4), 4);
                blue[samples] = pixel[0];
                green[samples] = pixel[1];
                red[samples] = pixel[2];
                samples += 1;
            }
        }
        io_surface::IOSurfaceUnlock(obj, READ_ONLY, std::ptr::null_mut());
        if samples == 0 {
            return None;
        }
        let median = samples / 2;
        red[..samples].sort_unstable();
        green[..samples].sort_unstable();
        blue[..samples].sort_unstable();
        let (red, green, blue) = (red[median], green[median], blue[median]);
        Some(u32::from_be_bytes([0, red, green, blue]))
    }
}
