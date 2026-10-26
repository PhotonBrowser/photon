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

use gpui::Window;
use io_surface::IOSurface;
use objc2::{MainThreadMarker, msg_send, rc::Retained, runtime::AnyObject};
use objc2_core_graphics::CGColor;
use objc2_foundation::{NSDictionary, NSNull, NSPoint, NSRect, NSSize, NSString};
use objc2_quartz_core::{CALayer, kCACornerCurveContinuous, kCAGravityResize};

use super::appkit::appkit_host_view;
use crate::platform::PagePlacement;

/// Identifies a frame by Engine backing, generation and frame number.
pub(super) type FrameIdentity = (u64, u64, u64);

pub(super) struct PageLayer {
    layer: Retained<CALayer>,
    /// Retained until a later transaction replaces the layer's contents.
    current_surface: RefCell<Option<IOSurface>>,
    current_identity: Cell<Option<FrameIdentity>>,
    contents_fraction: Cell<(f64, f64)>,
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
            "contents",
            "contentsRect",
            "contentsScale",
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
            layer.setContentsGravity(kCAGravityResize);
        }
        Self {
            layer,
            current_surface: RefCell::new(None),
            current_identity: Cell::new(None),
            contents_fraction: Cell::new((1.0, 1.0)),
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
        let host_layer = host
            .layer()
            .expect("GPUI's AppKit host view must have a Core Animation layer");
        let parent_layer = host_layer
            .superlayer()
            .expect("GPUI's AppKit host layer must have a superlayer");
        let layer = &self.layer;
        let attached = layer
            .superlayer()
            .is_some_and(|parent| std::ptr::eq(&*parent, &*parent_layer));

        let bounds = placement.bounds;
        let host_page_bounds = NSRect::new(
            NSPoint::new(
                f64::from(bounds.origin.x.as_f32()),
                f64::from(bounds.origin.y.as_f32()),
            ),
            NSSize::new(
                f64::from(bounds.size.width.as_f32()),
                f64::from(bounds.size.height.as_f32()),
            ),
        );
        let frame = parent_layer.convertRect_fromLayer(host_page_bounds, Some(&host_layer));
        let flipped = parent_layer.isGeometryFlipped();
        if attached && !layer.isHidden() && self.placed.get() == Some((placement, frame, flipped)) {
            return;
        }

        if !attached {
            // Directly beneath GPUI's layer. A page attached later, like a
            // link preview over its page, lies above those attached before.
            layer.removeFromSuperlayer();
            parent_layer.insertSublayer_below(layer, Some(&host_layer));
        }
        layer.setGeometryFlipped(flipped);
        layer.setContentsRect(contents_rect(layer, self.contents_fraction.get()));
        layer.setFrame(frame);
        layer.setCornerRadius(f64::from(placement.corner_radius));
        layer.setContentsScale(f64::from(placement.scale_factor));
        // Shown until the first frame arrives, as the page's own background.
        let [_, red, green, blue] = placement.background.to_be_bytes();
        let background = CGColor::new_srgb(
            f64::from(red) / 255.0,
            f64::from(green) / 255.0,
            f64::from(blue) / 255.0,
            1.0,
        );
        layer.setBackgroundColor(Some(&background));
        layer.setHidden(false);
        self.placed.set(Some((placement, frame, flipped)));
    }

    /// Hides the layer while its page is not shown, as for a background tab.
    /// A hidden page gets no GPUI prepaint, so `place` cannot hide it, and its
    /// last frame would show through the next page's hole. The next `place`
    /// shows it again.
    pub(super) fn hide(&self) {
        self.layer.setHidden(true);
    }

    /// Removes the layer from its window, as when the page closes.
    pub(super) fn detach(&self) {
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

/// The painted part of the surface, measured from its visual top-left.
fn contents_rect(layer: &CALayer, (width, height): (f64, f64)) -> NSRect {
    let y = if layer.contentsAreFlipped() {
        0.0
    } else {
        1.0 - height
    };
    NSRect::new(NSPoint::new(0.0, y), NSSize::new(width, height))
}
