//! AppKit views hosted as native children of GPUI's macOS window view.
//!
//! The native view sits above GPUI's Metal-backed content, so it receives
//! AppKit input and accessibility behavior without copying or repainting its
//! pixels through GPUI. Its frame and attachment follow the GPUI element tree.

use std::{
    cell::RefCell,
    ffi::c_void,
    ptr::NonNull,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};

use gpui::{
    App, AsyncApp, Bounds, DispatchPhase, Element, ElementId, GlobalElementId, InspectorElementId,
    IntoElement, LayoutId, MouseButton, MouseDownEvent, MouseUpEvent, Pixels, Refineable, Size,
    Style, StyleRefinement, Styled, Visibility, Window, px,
};
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
    rc::Retained,
    runtime::{AnyClass, AnyObject},
    sel,
};
use objc2_app_kit::{
    NSApplication, NSBezelStyle, NSButton, NSButtonType, NSCellImagePosition, NSGlassEffectView,
    NSGlassEffectViewStyle, NSImage, NSSegmentDistribution, NSSegmentStyle,
    NSSegmentSwitchTracking, NSSegmentedControl, NSView, NSWindowOrderingMode,
};
use objc2_core_graphics::CGColor;
use objc2_foundation::{
    NSArray, NSDictionary, NSNull, NSObject, NSPoint, NSRect, NSSize, NSString,
};
use objc2_quartz_core::CAGradientLayer;
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

static NEXT_NATIVE_VIEW_ID: AtomicU64 = AtomicU64::new(1);

/// A retained AppKit view that can be mounted in a GPUI layout tree.
///
/// Create these on GPUI's main thread. The view is attached when its element
/// appears and detached when GPUI drops that element's retained state.
#[derive(Clone)]
pub struct AppKitView {
    inner: Rc<AppKitViewInner>,
}

struct AppKitViewInner {
    view: Retained<NSView>,
    intrinsic_size_view: Retained<NSView>,
    glass_view: Option<Retained<NSGlassEffectView>>,
    glass_content_view: Option<Retained<NSView>>,
    id: u64,
}

impl AppKitView {
    fn from_retained(view: Retained<NSView>) -> Self {
        Self::from_parts(view.clone(), view, None, None)
    }

    fn from_button(button: &Retained<NSButton>) -> Self {
        // SAFETY: NSButton inherits from NSView, and the retained pointer stays
        // valid for the lifetime of this wrapper.
        let view =
            unsafe { Retained::<NSView>::retain(Retained::as_ptr(button).cast_mut().cast()) }
                .expect("an initialized NSButton is non-null");
        Self::from_retained(view)
    }

    fn from_glass(
        glass: Retained<NSGlassEffectView>,
        content_view: Retained<NSView>,
        intrinsic_size_view: Retained<NSView>,
    ) -> Self {
        // SAFETY: NSGlassEffectView inherits from NSView, and its retained
        // pointer stays valid for this wrapper's lifetime.
        let view =
            unsafe { Retained::<NSView>::retain(Retained::as_ptr(&glass).cast_mut().cast()) }
                .expect("an initialized NSGlassEffectView is non-null");
        Self::from_parts(view, intrinsic_size_view, Some(glass), Some(content_view))
    }

    fn from_parts(
        view: Retained<NSView>,
        intrinsic_size_view: Retained<NSView>,
        glass_view: Option<Retained<NSGlassEffectView>>,
        glass_content_view: Option<Retained<NSView>>,
    ) -> Self {
        Self {
            inner: Rc::new(AppKitViewInner {
                view,
                intrinsic_size_view,
                glass_view,
                glass_content_view,
                id: NEXT_NATIVE_VIEW_ID.fetch_add(1, Ordering::Relaxed),
            }),
        }
    }

    /// Wraps an owned Objective-C `NSView` pointer.
    ///
    /// # Safety
    ///
    /// `view` must be a valid `NSView` subclass pointer with a +1 retain count
    /// that this call may consume. The view and this wrapper must only be used
    /// on AppKit's main thread.
    pub unsafe fn from_raw_owned(view: NonNull<c_void>) -> Option<Self> {
        let _main_thread = MainThreadMarker::new()?;
        // SAFETY: the caller promises the pointer is a retained NSView and
        // transfers that +1 retain to this wrapper.
        let view = unsafe { Retained::<NSView>::from_raw(view.as_ptr().cast()) }?;
        Some(Self::from_retained(view))
    }

    /// Builds a layout element for this view.
    pub fn element(&self) -> AppKitViewElement {
        AppKitViewElement {
            view: self.clone(),
            style: StyleRefinement::default(),
        }
    }
}

/// A GPUI element that hosts an AppKit `NSView`.
///
/// GPUI owns layout and element lifetime. AppKit owns drawing, input, and
/// accessibility for the hosted view. GPUI clipping and paint order do not
/// apply to the native child view.
pub struct AppKitViewElement {
    view: AppKitView,
    style: StyleRefinement,
}

impl Styled for AppKitViewElement {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl IntoElement for AppKitViewElement {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

impl Element for AppKitViewElement {
    type RequestLayoutState = Style;
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        Some(ElementId::named_usize(
            "appkit-view",
            self.view.inner.id as usize,
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
        _cx: &mut App,
    ) -> (LayoutId, Self::RequestLayoutState) {
        let mut style = Style::default();
        style.refine(&self.style);

        let intrinsic_size_view = self.view.inner.intrinsic_size_view.clone();
        let layout = window.request_measured_layout(style.clone(), move |known, _, _, _| {
            // `intrinsicContentSize` is the AppKit sizing contract for native
            // controls. Custom NSViews can instead be sized by the caller.
            let size: NSSize = unsafe { msg_send![&*intrinsic_size_view, intrinsicContentSize] };
            Size {
                width: known
                    .width
                    .unwrap_or_else(|| px(size.width.max(0.0) as f32)),
                height: known
                    .height
                    .unwrap_or_else(|| px(size.height.max(0.0) as f32)),
            }
        });
        (layout, style)
    }

    fn prepaint(
        &mut self,
        id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        style: &mut Self::RequestLayoutState,
        _window: &mut Window,
        _cx: &mut App,
    ) -> Self::PrepaintState {
        let global_id = id.expect("AppKit view elements always have a stable id");
        let view = self.view.clone();
        let visible = style.visibility == Visibility::Visible;
        let alpha = style.opacity.unwrap_or(1.0).clamp(0.0, 1.0);

        // Element state survives consecutive frames, then drops automatically
        // when the element leaves the GPUI tree. Its Drop detaches the native
        // view before the owning AppKit component is released.
        _window.with_element_state(global_id, |state: Option<NativeAttachment>, window| {
            let mut attachment = state.unwrap_or_else(|| NativeAttachment::new(view));
            if let Some(host) = appkit_host_view(window) {
                // SAFETY: GPUI's live AppKit window owns this view throughout
                // this prepaint call.
                attachment.place(unsafe { host.as_ref() }, bounds, visible, alpha);
            } else {
                attachment.detach();
            }
            ((), attachment)
        });
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _style: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        _window: &mut Window,
        _cx: &mut App,
    ) {
        // The native NSView paints itself in AppKit's view hierarchy.
    }
}

struct NativeAttachment {
    view: AppKitView,
    host: Option<NonNull<NSView>>,
}

impl NativeAttachment {
    fn new(view: AppKitView) -> Self {
        Self { view, host: None }
    }

    fn place(&mut self, host: &NSView, bounds: Bounds<Pixels>, visible: bool, alpha: f32) {
        let native_view = &self.view.inner.view;
        // SAFETY: AppKit's view hierarchy is main-thread-only. Holding the
        // returned Retained value keeps the current parent alive while moving
        // the child if the element changes GPUI windows.
        let current_parent = unsafe { native_view.superview() };
        let same_parent = current_parent
            .as_ref()
            .is_some_and(|parent| std::ptr::eq(&**parent, host));
        if !same_parent {
            native_view.removeFromSuperview();
            host.addSubview(native_view);
        }
        self.host = Some(NonNull::from(host));

        let parent_bounds = host.bounds();
        let y = if host.isFlipped() {
            parent_bounds.origin.y + bounds.origin.y.as_f32() as f64
        } else {
            parent_bounds.origin.y + parent_bounds.size.height
                - bounds.origin.y.as_f32() as f64
                - bounds.size.height.as_f32() as f64
        };
        native_view.setFrame(NSRect::new(
            NSPoint::new(parent_bounds.origin.x + bounds.origin.x.as_f32() as f64, y),
            NSSize::new(
                bounds.size.width.as_f32() as f64,
                bounds.size.height.as_f32() as f64,
            ),
        ));
        native_view.setAlphaValue(alpha as f64);
        native_view.setHidden(
            !visible || bounds.size.width == Pixels::ZERO || bounds.size.height == Pixels::ZERO,
        );
        if let Some(glass) = &self.view.inner.glass_view {
            glass.setCornerRadius(bounds.size.height.as_f32() as f64 / 2.0);
        }
        if let Some(content) = &self.view.inner.glass_content_view {
            content.setFrame(native_view.bounds());
        }
    }

    fn detach(&mut self) {
        let Some(host) = self.host.take() else {
            return;
        };
        // SAFETY: AppKit view hierarchy access stays on the main thread, and
        // `superview` returns a retained reference for this comparison.
        let current_parent = unsafe { self.view.inner.view.superview() };
        if current_parent
            .as_ref()
            .is_some_and(|parent| std::ptr::eq(&**parent, host.as_ptr()))
        {
            self.view.inner.view.removeFromSuperview();
        }
    }
}

impl Drop for NativeAttachment {
    fn drop(&mut self) {
        self.detach();
    }
}

pub(super) fn appkit_host_view(window: &Window) -> Option<NonNull<NSView>> {
    let handle = HasWindowHandle::window_handle(window).ok()?;
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return None;
    };
    NonNull::new(handle.ns_view.as_ptr().cast())
}

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
}

impl AppKitGlassBackdrop {
    /// Returns `None` before macOS 26, or off the AppKit main thread, so the
    /// caller keeps its own painted surface.
    pub fn new() -> Option<Self> {
        let mtm = MainThreadMarker::new()?;
        AnyClass::get(c"NSGlassEffectView")?;
        let container = NSView::new(mtm);
        container.setWantsLayer(true);
        let container_layer = container.layer()?;
        container_layer.setMasksToBounds(true);
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
            }),
            corner_radius: Pixels::ZERO,
            window_tint: GlassWindowTint::default(),
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
    type PrepaintState = ();

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
        let inner = self.inner.clone();
        let radius = self.corner_radius;
        let tint = self.window_tint.clone();
        let viewport = window.viewport_size();
        window.with_element_state(global_id, |state: Option<GlassAttachment>, window| {
            let mut attachment = state.unwrap_or_else(|| GlassAttachment {
                inner,
                parent: None,
            });
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
                    },
                );
            } else {
                attachment.detach();
            }
            ((), attachment)
        });
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _style: &mut Self::RequestLayoutState,
        _prepaint: &mut Self::PrepaintState,
        window: &mut Window,
        _cx: &mut App,
    ) {
        // Clear whatever GPUI painted here before, such as the window tint,
        // so the glass shows through as itself. The element's later children
        // still draw over it.
        if !self.inner.container.isHidden() {
            window.paint_cutout(bounds, self.corner_radius.into(), 0.0);
        }
        // The glass lies beneath GPUI's view and never receives AppKit hits,
        // so hand it the presses GPUI sees over it.
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
}

struct GlassPlacement<'a> {
    bounds: Bounds<Pixels>,
    viewport: Size<Pixels>,
    visible: bool,
    alpha: f32,
    radius: Pixels,
    tint: &'a GlassWindowTint,
}

impl GlassAttachment {
    fn place(&mut self, host: &NSView, placement: GlassPlacement<'_>) {
        let GlassPlacement {
            bounds,
            viewport,
            visible,
            alpha,
            radius,
            tint,
        } = placement;
        // SAFETY: AppKit view hierarchy access runs on the main thread.
        let Some(parent) = (unsafe { host.superview() }) else {
            self.detach();
            return;
        };
        let container = &self.inner.container;
        // SAFETY: as above.
        let current_parent = unsafe { container.superview() };
        if !current_parent
            .as_ref()
            .is_some_and(|current| std::ptr::eq(&**current, &*parent))
        {
            container.removeFromSuperview();
            // Directly beneath GPUI's view, so its contents draw over the glass.
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
        container.setFrame(host.convertRect_toView(frame, Some(&parent)));
        container.setAlphaValue(alpha as f64);
        container.setHidden(
            !visible || bounds.size.width == Pixels::ZERO || bounds.size.height == Pixels::ZERO,
        );
        if let Some(layer) = container.layer() {
            layer.setCornerRadius(radius.as_f32() as f64);
        }
        let glass = &self.inner.glass;
        glass.setFrame(container.bounds());
        glass.setCornerRadius(radius.as_f32() as f64);
        self.place_tint(bounds, viewport, container.isFlipped(), tint);
    }

    /// Lays the window-sized tint gradient under the field so the slice the
    /// field covers matches what GPUI paints around it.
    fn place_tint(
        &self,
        bounds: Bounds<Pixels>,
        viewport: Size<Pixels>,
        flipped: bool,
        tint: &GlassWindowTint,
    ) {
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
    fn drop(&mut self) {
        self.detach();
    }
}

/// The native bezel appearance for an AppKit button.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum AppKitButtonStyle {
    /// AppKit Liquid Glass on macOS 26+, with its native toolbar bezel fallback.
    #[default]
    LiquidGlass,
    /// AppKit's toolbar button appearance on every supported macOS release.
    Toolbar,
    /// A borderless native button.
    Borderless,
}

/// A native AppKit button that can be used directly as a GPUI child element.
///
/// Keep the button in a retained GPUI view/entity and render clones of it. The
/// button's intrinsic AppKit size is used unless the GPUI parent constrains it.
#[derive(Clone)]
pub struct AppKitButton {
    inner: Rc<AppKitButtonInner>,
    style: StyleRefinement,
}

struct AppKitButtonInner {
    view: RefCell<AppKitView>,
    button: Retained<NSButton>,
    target: Retained<NativeControlActionTarget>,
}

impl AppKitButton {
    /// Creates a native symbol button. Returns `None` if AppKit does not know
    /// the requested SF Symbol.
    pub fn symbol(symbol: &str, accessibility_label: &str) -> Option<Self> {
        let mtm = MainThreadMarker::new()?;
        let symbol_name = NSString::from_str(symbol);
        let accessibility_label = NSString::from_str(accessibility_label);
        let image = NSImage::imageWithSystemSymbolName_accessibilityDescription(
            &symbol_name,
            Some(&accessibility_label),
        )?;
        Some(Self::from_parts(
            mtm,
            "",
            Some(&image),
            AppKitButtonStyle::default(),
        ))
    }

    /// Creates a native text button.
    pub fn title(title: &str) -> Option<Self> {
        let mtm = MainThreadMarker::new()?;
        Some(Self::from_parts(
            mtm,
            title,
            None,
            AppKitButtonStyle::default(),
        ))
    }

    fn from_parts(
        mtm: MainThreadMarker,
        title: &str,
        image: Option<&NSImage>,
        style: AppKitButtonStyle,
    ) -> Self {
        let target = NativeControlActionTarget::new(mtm);
        let title = NSString::from_str(title);
        // SAFETY: `target` is the action class declared below and `activate:`
        // is its registered selector.
        let button = unsafe {
            NSButton::buttonWithTitle_target_action(
                &title,
                Some(&*target),
                Some(sel!(activate:)),
                mtm,
            )
        };
        button.setButtonType(NSButtonType::MomentaryPushIn);
        button.setImage(image);
        if image.is_some() {
            button.setImagePosition(NSCellImagePosition::ImageOnly);
        }
        let view = appkit_button_view(&button, style, mtm);

        Self {
            inner: Rc::new(AppKitButtonInner {
                view: RefCell::new(view),
                button,
                target,
            }),
            style: StyleRefinement::default(),
        }
    }

    /// Registers a Rust callback for the native button action.
    ///
    /// The handler is scheduled onto GPUI's foreground executor, so updating
    /// entities does not re-enter a GPUI draw or AppKit event callback.
    pub fn on_click(self, app: AsyncApp, action: impl FnMut(&mut App) + 'static) -> Self {
        let action = Rc::new(RefCell::new(action));
        self.inner.target.set_action(move |_| {
            let action = action.clone();
            let task = app.spawn(async move |cx| {
                cx.update(|cx| (action.borrow_mut())(cx));
            });
            task.detach();
        });
        self
    }

    /// Chooses the native appearance. Liquid Glass falls back to a system
    /// toolbar bezel before macOS 26.
    pub fn appearance(self, style: AppKitButtonStyle) -> Self {
        let Some(mtm) = MainThreadMarker::new() else {
            return self;
        };
        *self.inner.view.borrow_mut() = appkit_button_view(&self.inner.button, style, mtm);
        self
    }

    /// Enables or disables the native control.
    pub fn enabled(self, enabled: bool) -> Self {
        self.inner.button.setEnabled(enabled);
        self
    }

    /// Updates the native tooltip shown on hover and exposed to assistive
    /// technologies when AppKit derives its help text from the control.
    pub fn tooltip(self, tooltip: &str) -> Self {
        let tooltip = NSString::from_str(tooltip);
        self.inner.button.setToolTip(Some(&tooltip));
        self
    }
}

/// A native AppKit segmented control used as a tab bar in a GPUI layout.
///
/// AppKit owns selection and accessibility for the control. The selected
/// index is delivered to GPUI through `on_change`, where the host can switch
/// its content.
#[derive(Clone)]
pub struct AppKitTabBar {
    inner: Rc<AppKitTabBarInner>,
    style: StyleRefinement,
}

struct AppKitTabBarInner {
    view: AppKitView,
    control: Retained<NSSegmentedControl>,
    target: Retained<NativeControlActionTarget>,
    segment_count: usize,
}

impl AppKitTabBar {
    /// Creates a native, single-selection tab bar with the given labels.
    /// Returns `None` if called off the AppKit main thread or with no tabs.
    pub fn new(labels: &[&str]) -> Option<Self> {
        let mtm = MainThreadMarker::new()?;
        if labels.is_empty() {
            return None;
        }

        let target = NativeControlActionTarget::new(mtm);
        let control = NSSegmentedControl::new(mtm);
        control.setSegmentCount(labels.len() as isize);
        for (index, label) in labels.iter().enumerate() {
            let label = NSString::from_str(label);
            control.setLabel_forSegment(&label, index as isize);
        }
        control.setTrackingMode(NSSegmentSwitchTracking::SelectOne);
        control.setSegmentStyle(NSSegmentStyle::Automatic);
        control.setSegmentDistribution(NSSegmentDistribution::FillEqually);
        control.setSelectedSegment(0);
        // SAFETY: `target` implements the activate: selector and receives this
        // native control's action messages.
        unsafe {
            control.setTarget(Some(&*target));
            control.setAction(Some(sel!(activate:)));
        }

        // NSSegmentedControl inherits from NSView; retain a view-typed handle
        // both for the GPUI host and for the optional glass content view.
        // SAFETY: `control` is an initialized NSView subclass.
        let control_view =
            unsafe { Retained::<NSView>::retain(Retained::as_ptr(&control).cast_mut().cast()) }?;
        let view = if set_tab_role(&control) {
            // macOS 27's native tabs role opts into its purpose-built tab
            // appearance and VoiceOver semantics.
            AppKitView::from_retained(control_view)
        } else if AnyClass::get(c"NSGlassEffectView").is_some() {
            // macOS 26 doesn't have a tabs role, so provide the dynamic glass
            // material around its native segmented control.
            let glass = NSGlassEffectView::new(mtm);
            glass.setStyle(NSGlassEffectViewStyle::Regular);
            glass.setContentView(Some(&control_view));
            enable_interactive_glass(&glass);
            AppKitView::from_glass(glass, control_view.clone(), control_view)
        } else {
            AppKitView::from_retained(control_view)
        };

        Some(Self {
            inner: Rc::new(AppKitTabBarInner {
                view,
                control,
                target,
                segment_count: labels.len(),
            }),
            style: StyleRefinement::default(),
        })
    }

    /// Sets the selected tab by index. Out-of-range indices select the last
    /// tab, matching the control's available range.
    pub fn set_selected_index(&self, index: usize) {
        let index = index.min(self.inner.segment_count - 1);
        self.inner.control.setSelectedSegment(index as isize);
    }

    /// Returns the selected tab index, if a tab is selected.
    pub fn selected_index(&self) -> Option<usize> {
        let index = self.inner.control.selectedSegment();
        (index >= 0).then_some(index as usize)
    }

    /// Registers a GPUI callback for native tab selection changes.
    pub fn on_change(self, app: AsyncApp, action: impl FnMut(usize, &mut App) + 'static) -> Self {
        let action = Rc::new(RefCell::new(action));
        let control = self.inner.control.clone();
        self.inner.target.set_action(move |_| {
            let selected = control.selectedSegment();
            if selected < 0 {
                return;
            }
            let action = action.clone();
            let task = app.spawn(async move |cx| {
                cx.update(|cx| (action.borrow_mut())(selected as usize, cx));
            });
            task.detach();
        });
        self
    }
}

impl Styled for AppKitTabBar {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl IntoElement for AppKitTabBar {
    type Element = AppKitViewElement;

    fn into_element(self) -> Self::Element {
        AppKitViewElement {
            view: self.inner.view.clone(),
            style: self.style,
        }
    }
}

impl Styled for AppKitButton {
    fn style(&mut self) -> &mut StyleRefinement {
        &mut self.style
    }
}

impl IntoElement for AppKitButton {
    type Element = AppKitViewElement;

    fn into_element(self) -> Self::Element {
        AppKitViewElement {
            view: self.inner.view.borrow().clone(),
            style: self.style,
        }
    }
}

fn apply_button_style(button: &NSButton, style: AppKitButtonStyle) {
    match style {
        AppKitButtonStyle::LiquidGlass if AnyClass::get(c"NSGlassEffectView").is_some() => {
            button.setBordered(false);
        }
        AppKitButtonStyle::LiquidGlass | AppKitButtonStyle::Toolbar => {
            button.setBordered(true);
            button.setBezelStyle(NSBezelStyle::Toolbar);
        }
        AppKitButtonStyle::Borderless => button.setBordered(false),
    }
}

fn appkit_button_view(
    button: &Retained<NSButton>,
    style: AppKitButtonStyle,
    mtm: MainThreadMarker,
) -> AppKitView {
    apply_button_style(button, style);
    if style != AppKitButtonStyle::LiquidGlass || AnyClass::get(c"NSGlassEffectView").is_none() {
        return AppKitView::from_button(button);
    }

    let glass = NSGlassEffectView::new(mtm);
    glass.setStyle(NSGlassEffectViewStyle::Regular);
    // SAFETY: NSButton inherits from NSView and is retained by the caller.
    let button_view =
        unsafe { Retained::<NSView>::retain(Retained::as_ptr(button).cast_mut().cast()) }
            .expect("an initialized NSButton is non-null");
    glass.setContentView(Some(&button_view));
    enable_interactive_glass(&glass);
    AppKitView::from_glass(glass, button_view.clone(), button_view)
}

fn set_tab_role(control: &NSSegmentedControl) -> bool {
    // NSSegmentedControlRoleTabs is available in macOS 27. Query the selector
    // so older systems keep their supported AppKit tab appearance.
    let selector = sel!(setRole:);
    let supports_tab_role: bool = unsafe { msg_send![control, respondsToSelector: selector] };
    if supports_tab_role {
        // SAFETY: NSSegmentedControlRoleTabs has the value 1 in the macOS 27
        // SDK, and respondsToSelector confirmed this setter is available.
        let _: () = unsafe { msg_send![control, setRole: 1_isize] };
    }
    supports_tab_role
}

fn enable_interactive_glass(glass: &NSGlassEffectView) {
    // macOS 27 adds a localized press response to native glass. Query the
    // selector so this also works on macOS 26.
    let interactive_selector = sel!(setEffectIsInteractive:);
    let supports_interactive_glass: bool =
        unsafe { msg_send![glass, respondsToSelector: interactive_selector] };
    if supports_interactive_glass {
        let _: () = unsafe { msg_send![glass, setEffectIsInteractive: true] };
    }
}

type ControlAction = Box<dyn FnMut(&AnyObject)>;

#[derive(Default)]
struct NativeControlActionTargetIvars {
    action: RefCell<Option<ControlAction>>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "PhotonAppKitControlActionTarget"]
    #[ivars = NativeControlActionTargetIvars]
    struct NativeControlActionTarget;

    impl NativeControlActionTarget {
        #[unsafe(method(activate:))]
        fn activate(&self, sender: &AnyObject) {
            if let Some(action) = self.ivars().action.borrow_mut().as_mut() {
                action(sender);
            }
        }
    }
);

impl NativeControlActionTarget {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(NativeControlActionTargetIvars::default());
        // SAFETY: NSObject's init is the designated initializer for this
        // action target, which adds no custom initialization requirements.
        unsafe { msg_send![super(this), init] }
    }

    fn set_action(&self, action: impl FnMut(&AnyObject) + 'static) {
        *self.ivars().action.borrow_mut() = Some(Box::new(action));
    }
}
