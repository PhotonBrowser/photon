//! AppKit views hosted as native children of GPUI's macOS window view.
//!
//! The native view sits above GPUI's Metal-backed content, so it receives
//! AppKit input and accessibility behavior without copying or repainting its
//! pixels through GPUI. Its frame and attachment follow the GPUI element tree.
#![allow(deprecated)]

use std::{
    cell::{Cell, RefCell},
    ffi::c_void,
    ptr::NonNull,
    rc::Rc,
    sync::atomic::{AtomicU64, Ordering},
};

use gpui::{
    App, AsyncApp, Bounds, Element, ElementId, GlobalElementId, InspectorElementId, IntoElement,
    LayoutId, Pixels, Refineable, Size, Style, StyleRefinement, Styled, Visibility, Window, px,
};
use io_surface::IOSurface;
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send,
    rc::Retained,
    runtime::{AnyClass, AnyObject},
    sel,
};
use objc2_app_kit::{
    NSBezelStyle, NSButton, NSButtonType, NSCellImagePosition, NSGlassEffectView,
    NSGlassEffectViewStyle, NSImage, NSSegmentDistribution, NSSegmentStyle,
    NSSegmentSwitchTracking, NSSegmentedControl, NSView,
};
use objc2_core_graphics::CGColor;
use objc2_foundation::{NSObject, NSPoint, NSRect, NSSize, NSString};
use objc2_quartz_core::{CALayer, CATransaction, kCACornerCurveContinuous, kCAGravityResize};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

static NEXT_NATIVE_VIEW_ID: AtomicU64 = AtomicU64::new(1);

/// A Core Animation layer that displays an Engine IOSurface directly.
///
/// The layer sits beneath GPUI's layer, which cuts a hole where the page is
/// (see `Window::paint_cutout`). Core Animation and the window server
/// composite the page without copying it through GPUI, and anything GPUI
/// draws over the page, such as a tooltip or menu, stays above it.
#[derive(Clone)]
pub struct AppKitIOSurfaceLayer {
    inner: Rc<IOSurfaceLayerInner>,
}

struct IOSurfaceLayerInner {
    layer: Retained<CALayer>,
    current_surface: RefCell<Option<IOSurface>>,
    current_identity: RefCell<Option<(u64, u64, u64)>>,
    contents_fraction: Cell<(f64, f64)>,
}

impl AppKitIOSurfaceLayer {
    /// Creates a layer wrapper on GPUI's AppKit thread.
    pub fn new() -> Self {
        let _main_thread = MainThreadMarker::new()
            .expect("Core Animation layers must be created on the AppKit main thread");
        let layer = CALayer::layer();
        layer.setMasksToBounds(true);
        // SAFETY: Core Animation exposes these process-wide immutable values.
        unsafe {
            layer.setCornerCurve(kCACornerCurveContinuous);
            layer.setContentsGravity(kCAGravityResize);
        }
        Self {
            inner: Rc::new(IOSurfaceLayerInner {
                layer,
                current_surface: RefCell::new(None),
                current_identity: RefCell::new(None),
                contents_fraction: Cell::new((1.0, 1.0)),
            }),
        }
    }

    /// Shows a frame, without painting page pixels through GPUI.
    pub fn set_surface(
        &self,
        identity: (u64, u64, u64),
        surface: &IOSurface,
        painted_width: i32,
        painted_height: i32,
    ) {
        if self.shows(identity) {
            return;
        }
        if painted_width <= 0 || painted_height <= 0 {
            self.clear_surface();
            return;
        }

        let _main_thread = MainThreadMarker::new()
            .expect("Core Animation layers must be updated on the AppKit main thread");
        let width = unsafe { io_surface::IOSurfaceGetWidth(surface.obj) } as f64;
        let height = unsafe { io_surface::IOSurfaceGetHeight(surface.obj) } as f64;
        if width <= 0.0 || height <= 0.0 {
            self.clear_surface();
            return;
        }

        // A resize can pad the surface beyond what was painted; show only
        // the painted part.
        let contents_fraction = (
            f64::from(painted_width).min(width) / width,
            f64::from(painted_height).min(height) / height,
        );
        self.inner.contents_fraction.set(contents_fraction);
        CATransaction::begin();
        CATransaction::setDisableActions(true);
        self.inner.layer.setContentsRect(iosurface_contents_rect(
            &self.inner.layer,
            contents_fraction,
        ));
        // IOSurface is a Core Foundation object accepted by CALayer.contents.
        // Keep our own retained reference until a later transaction replaces it.
        let contents = unsafe { &*surface.obj.cast::<AnyObject>() };
        unsafe { self.inner.layer.setContents(Some(contents)) };
        // Ladybird's Qt view also asks for a window display cycle, to commit
        // with Qt's own backing store and keep Qt routing mouse events to its
        // native view. AppKit routes events to GPUI's view by view, not by
        // layer, and GPUI has nothing new to draw.
        CATransaction::commit();
        *self.inner.current_surface.borrow_mut() = Some(surface.clone());
        *self.inner.current_identity.borrow_mut() = Some(identity);
    }

    /// Whether the layer already shows this frame.
    pub fn shows(&self, identity: (u64, u64, u64)) -> bool {
        *self.inner.current_identity.borrow() == Some(identity)
    }

    /// Clears the layer while GPUI draws the page instead.
    pub fn clear_surface(&self) {
        if self.inner.current_identity.borrow().is_none() {
            return;
        }
        let _main_thread = MainThreadMarker::new()
            .expect("Core Animation layers must be updated on the AppKit main thread");
        CATransaction::begin();
        CATransaction::setDisableActions(true);
        unsafe { self.inner.layer.setContents(None) };
        CATransaction::commit();
        *self.inner.current_surface.borrow_mut() = None;
        *self.inner.current_identity.borrow_mut() = None;
        self.inner.contents_fraction.set((1.0, 1.0));
    }

    /// Positions the layer beneath the page's hole in GPUI's layer, from the
    /// page's GPUI prepaint.
    pub fn place(&self, window: &Window, placement: IOSurfaceLayerPlacement) {
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
        let layer = &self.inner.layer;
        let attached = layer
            .superlayer()
            .is_some_and(|parent| std::ptr::eq(&*parent, &*parent_layer));
        CATransaction::begin();
        CATransaction::setDisableActions(true);
        if !attached {
            // Directly beneath GPUI's layer. A page attached later, like a
            // link preview over its page, lies above those attached before.
            layer.removeFromSuperlayer();
            parent_layer.insertSublayer_below(layer, Some(&host_layer));
        }

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
        layer.setGeometryFlipped(parent_layer.isGeometryFlipped());
        layer.setContentsRect(iosurface_contents_rect(
            layer,
            self.inner.contents_fraction.get(),
        ));
        layer.setFrame(parent_layer.convertRect_fromLayer(host_page_bounds, Some(&host_layer)));
        layer.setCornerRadius(f64::from(placement.corner_radius));
        layer.setContentsScale(f64::from(placement.scale_factor));
        // Shown until the first frame arrives, as the page's own background.
        let [red, green, blue] = placement.background_rgb.to_be_bytes()[1..] else {
            unreachable!()
        };
        let background = CGColor::new_srgb(
            f64::from(red) / 255.0,
            f64::from(green) / 255.0,
            f64::from(blue) / 255.0,
            1.0,
        );
        layer.setBackgroundColor(Some(&background));
        layer.setHidden(false);
        CATransaction::commit();
    }

    /// Hides the layer when its page stops being shown, as for a background
    /// tab. A hidden page gets no GPUI prepaint, so `place` cannot hide it,
    /// and its last frame would show through the next page's hole. The next
    /// `place` shows it again.
    pub fn hide(&self) {
        if MainThreadMarker::new().is_none() {
            return;
        }
        CATransaction::begin();
        CATransaction::setDisableActions(true);
        self.inner.layer.setHidden(true);
        CATransaction::commit();
    }

    /// Removes this layer from the window when the page is torn down.
    pub fn detach(&self) {
        self.inner.layer.removeFromSuperlayer();
    }
}

/// Where a page's layer goes, from the page's GPUI layout.
pub struct IOSurfaceLayerPlacement {
    /// The page's bounds in the window.
    pub bounds: Bounds<Pixels>,
    pub corner_radius: f32,
    pub scale_factor: f32,
    /// The page's background, as `0xRRGGBB`.
    pub background_rgb: u32,
}

impl Default for AppKitIOSurfaceLayer {
    fn default() -> Self {
        Self::new()
    }
}

impl Drop for IOSurfaceLayerInner {
    fn drop(&mut self) {
        if MainThreadMarker::new().is_some() {
            self.layer.removeFromSuperlayer();
        }
    }
}

fn iosurface_contents_rect(layer: &CALayer, contents_fraction: (f64, f64)) -> NSRect {
    let (width_fraction, height_fraction) = contents_fraction;
    if layer.contentsAreFlipped() {
        NSRect::new(
            NSPoint::new(0.0, 0.0),
            NSSize::new(width_fraction, height_fraction),
        )
    } else {
        NSRect::new(
            NSPoint::new(0.0, 1.0 - height_fraction),
            NSSize::new(width_fraction, height_fraction),
        )
    }
}

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

fn appkit_host_view(window: &Window) -> Option<NonNull<NSView>> {
    let handle = HasWindowHandle::window_handle(window).ok()?;
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return None;
    };
    NonNull::new(handle.ns_view.as_ptr().cast())
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
