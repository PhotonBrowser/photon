//! The site information popover in native AppKit: an `NSPopover`, which is
//! Liquid Glass on macOS 26+, laid out like the GPUI popover other platforms
//! draw and filled from the same [`SiteInfo`].

use std::{
    cell::{Cell, RefCell},
    rc::Rc,
    time::{Duration, Instant},
};

use gpui::{App, AsyncApp, Pixels, Point, Window};
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, define_class, msg_send, rc::Retained,
    runtime::ProtocolObject, sel,
};
use objc2_app_kit::{
    NSAccessibility, NSBezelStyle, NSBox, NSBoxType, NSButton, NSButtonType, NSCellImagePosition,
    NSColor, NSFont, NSFontWeightRegular, NSFontWeightSemibold, NSImage,
    NSImageSymbolConfiguration, NSImageView, NSLayoutAttribute, NSLayoutConstraintOrientation,
    NSLineBreakMode, NSPopover, NSPopoverBehavior, NSPopoverDelegate, NSStackView,
    NSStackViewDistribution, NSStackViewGravity, NSTextField, NSUserInterfaceLayoutOrientation,
    NSView, NSViewController,
};
use objc2_foundation::{
    NSEdgeInsets, NSNotification, NSObject, NSObjectProtocol, NSPoint, NSRect, NSRectEdge, NSSize,
    NSString,
};

use super::appkit::{NativeControlActionTarget, appkit_host_view};
use crate::platform::metrics;
use crate::platform::site_info::{
    BACK_LABEL, CLOSE_LABEL, ConnectionStatus, SiteControl, SiteDetail, SiteInfo, TITLE,
    VIEW_CERTIFICATE_LABEL,
};

/// The native popover. Keep one per omnibox and update it as the site changes.
#[derive(Clone)]
pub struct AppKitSiteInfoPopover {
    inner: Rc<Inner>,
}

struct Inner {
    popover: Retained<NSPopover>,
    pages: Pages,
    controls: Vec<(SiteControl, Retained<NSButton>)>,
    heading: Retained<NSTextField>,
    connection: Retained<NSButton>,
    security: SecurityPage,
    shown: RefCell<Option<SiteInfo>>,
    delegate: Retained<PopoverDelegate>,
    /// AppKit holds its action targets weakly.
    _targets: Vec<Retained<NativeControlActionTarget>>,
}

/// The popover's two pages: the site, and its connection.
#[derive(Clone)]
struct Pages {
    popover: Retained<NSPopover>,
    controller: Retained<NSViewController>,
    site: Retained<NSView>,
    security: Retained<NSView>,
}

impl Pages {
    fn show(&self, page: &NSView) {
        self.controller.setView(page);
        self.popover.setContentSize(page.fittingSize());
    }
}

/// The connection page's fields, filled from the site.
struct SecurityPage {
    icon: Retained<NSImageView>,
    title: Retained<NSTextField>,
    site: Retained<NSTextField>,
    summary: Retained<NSTextField>,
}

impl AppKitSiteInfoPopover {
    /// `on_toggle` turns a control on or off; `on_close` follows the close
    /// button. Returns `None` off the AppKit main thread.
    pub fn new(
        app: AsyncApp,
        on_toggle: impl FnMut(SiteControl, &mut App) + 'static,
        mut on_close: impl FnMut(&mut App) + 'static,
    ) -> Option<Self> {
        let mtm = MainThreadMarker::new()?;
        let on_toggle = Rc::new(RefCell::new(on_toggle));
        let mut targets = Vec::new();

        let controller = NSViewController::new(mtm);
        let popover = NSPopover::new(mtm);
        popover.setBehavior(NSPopoverBehavior::Transient);
        popover.setAnimates(true);
        popover.setContentViewController(Some(&controller));
        let delegate = PopoverDelegate::new(mtm);
        popover.setDelegate(Some(ProtocolObject::from_ref(&*delegate)));
        let site_view = page(mtm);
        let (security_view, security, security_header) = security_page(mtm);
        let pages = Pages {
            popover: popover.clone(),
            controller,
            site: Retained::into_super(site_view.clone()),
            security: Retained::into_super(security_view),
        };

        let controls_row = stack(
            NSUserInterfaceLayoutOrientation::Horizontal,
            metrics::SITE_INFO_GAP,
            mtm,
        );
        controls_row.setDistribution(NSStackViewDistribution::FillEqually);
        let controls = SiteControl::ALL
            .into_iter()
            .map(|control| {
                let on_toggle = on_toggle.clone();
                let (button, target) = glass_button(
                    control.label(),
                    in_app(&app, move |cx| (on_toggle.borrow_mut())(control, cx)),
                    mtm,
                );
                button.setButtonType(NSButtonType::PushOnPushOff);
                set_height(&button, metrics::SITE_INFO_TILE_HEIGHT);
                controls_row.addArrangedSubview(&button);
                targets.push(target);
                (control, button)
            })
            .collect();

        let heading = label("", heading_font(), mtm);
        let site_section = stack(
            NSUserInterfaceLayoutOrientation::Vertical,
            metrics::SITE_INFO_GAP,
            mtm,
        );
        site_section.setAlignment(NSLayoutAttribute::Leading);
        site_section.addArrangedSubview(&heading);
        for detail in SiteDetail::ALL {
            site_section.addArrangedSubview(&detail_row(detail, mtm));
        }

        let show_security = pages.clone();
        let (connection, connection_target) =
            glass_button("", move || show_security.show(&show_security.security), mtm);
        connection.setImagePosition(NSCellImagePosition::ImageLeading);
        connection.setFont(Some(&NSFont::systemFontOfSize_weight(
            metrics::MENU_FONT_SIZE as f64,
            // SAFETY: AppKit's font weight constants are immutable statics.
            unsafe { NSFontWeightSemibold },
        )));
        set_height(&connection, metrics::SITE_INFO_FOOTER_HEIGHT);
        let (close, close_target) =
            glass_button(CLOSE_LABEL, in_app(&app, move |cx| on_close(cx)), mtm);
        round(&close, "xmark", CLOSE_LABEL);
        let footer = stack(
            NSUserInterfaceLayoutOrientation::Horizontal,
            metrics::SITE_INFO_GAP,
            mtm,
        );
        footer.addView_inGravity(&connection, NSStackViewGravity::Leading);
        footer.addView_inGravity(&close, NSStackViewGravity::Trailing);

        let show_site = pages.clone();
        let (back, back_target) =
            glass_button(BACK_LABEL, move || show_site.show(&show_site.site), mtm);
        round(&back, "chevron.left", BACK_LABEL);
        security_header.insertArrangedSubview_atIndex(&back, 0);
        targets.extend([connection_target, close_target, back_target]);

        let separator = NSBox::new(mtm);
        separator.setBoxType(NSBoxType::Separator);
        fill(
            &site_view,
            [&controls_row, &site_section, &separator, &footer],
        );
        site_view.setAccessibilityLabel(Some(&NSString::from_str(TITLE)));
        pages.show(&pages.site);

        Some(Self {
            inner: Rc::new(Inner {
                popover,
                pages,
                controls,
                heading,
                connection,
                security,
                shown: RefCell::new(None),
                delegate,
                _targets: targets,
            }),
        })
    }

    /// Opens the popover on its site page, pointing at `anchor` in `window`.
    pub fn show(&self, window: &Window, anchor: Point<Pixels>) {
        let Some(host) = appkit_host_view(window) else {
            return;
        };
        // SAFETY: the window handle gives its live AppKit view on the main thread.
        let host = unsafe { host.as_ref() };
        let x = f32::from(anchor.x) as f64;
        let y = f32::from(anchor.y) as f64;
        let y = if host.isFlipped() {
            y
        } else {
            host.bounds().size.height - y
        };
        self.inner.pages.show(&self.inner.pages.site);
        self.inner.popover.showRelativeToRect_ofView_preferredEdge(
            NSRect::new(NSPoint::new(x, y), NSSize::new(1.0, 1.0)),
            host,
            NSRectEdge::MaxY,
        );
    }

    pub fn close(&self) {
        // SAFETY: a nil sender is allowed, and this runs on the main thread.
        unsafe { self.inner.popover.performClose(None) };
    }

    pub fn is_shown(&self) -> bool {
        self.inner.popover.isShown()
    }

    /// Whether the popover closed within `window`, as a transient popover
    /// does on the very press that would toggle it, before the press's click
    /// arrives.
    pub fn closed_within(&self, window: Duration) -> bool {
        self.inner
            .delegate
            .ivars()
            .closed_at
            .get()
            .is_some_and(|closed| closed.elapsed() < window)
    }

    /// Shows `info`, touching AppKit only when it changed.
    pub fn update(&self, info: &SiteInfo) {
        if self.inner.shown.borrow().as_ref() == Some(info) {
            return;
        }
        let site = NSString::from_str(&info.site);
        self.inner.heading.setStringValue(&site);
        for (control, button) in &self.inner.controls {
            let state = control.state(info);
            let on = state == Some(true);
            button.setImage(symbol(control.symbol(on), control.label()).as_deref());
            button.setState(if on { 1 } else { 0 });
            button.setEnabled(state.is_some());
            button.setBezelColor(on.then(NSColor::controlAccentColor).as_deref());
        }

        let status = ConnectionStatus::of(info);
        let tint = match status {
            ConnectionStatus::Secure => NSColor::systemGreenColor(),
            ConnectionStatus::NotSecure => NSColor::systemRedColor(),
            ConnectionStatus::Unknown => NSColor::secondaryLabelColor(),
        };
        let image = symbol(status.symbol(), status.label());
        let connection = &self.inner.connection;
        connection.setTitle(&NSString::from_str(status.label()));
        connection.setImage(image.as_deref());
        connection.setContentTintColor(Some(&tint));
        connection.setAccessibilityLabel(Some(&NSString::from_str(status.title())));

        let security = &self.inner.security;
        security.icon.setImage(image.as_deref());
        security.icon.setContentTintColor(Some(&tint));
        security
            .title
            .setStringValue(&NSString::from_str(status.title()));
        security.site.setStringValue(&site);
        security
            .summary
            .setStringValue(&NSString::from_str(status.summary()));
        *self.inner.shown.borrow_mut() = Some(info.clone());
    }
}

#[derive(Default)]
struct PopoverDelegateIvars {
    closed_at: Cell<Option<Instant>>,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "PhotonSiteInfoPopoverDelegate"]
    #[ivars = PopoverDelegateIvars]
    struct PopoverDelegate;

    unsafe impl NSObjectProtocol for PopoverDelegate {}

    unsafe impl NSPopoverDelegate for PopoverDelegate {
        #[unsafe(method(popoverDidClose:))]
        fn popover_did_close(&self, _notification: &NSNotification) {
            self.ivars().closed_at.set(Some(Instant::now()));
        }
    }
);

impl PopoverDelegate {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(PopoverDelegateIvars::default());
        // SAFETY: NSObject's init is the designated initializer, and this
        // delegate adds no initialization requirements of its own.
        unsafe { msg_send![super(this), init] }
    }
}

impl Drop for Inner {
    fn drop(&mut self) {
        self.popover.close();
    }
}

/// The connection's page: its heading, the site with its lock, and what the
/// connection means. Returns the page, its fields, and the header row the
/// back button joins.
fn security_page(
    mtm: MainThreadMarker,
) -> (Retained<NSStackView>, SecurityPage, Retained<NSStackView>) {
    let header = stack(
        NSUserInterfaceLayoutOrientation::Horizontal,
        metrics::SITE_INFO_GAP,
        mtm,
    );
    let title = label("", heading_font(), mtm);
    header.addArrangedSubview(&title);

    let icon = NSImageView::new(mtm);
    let site = label(
        "",
        NSFont::systemFontOfSize(metrics::MENU_FONT_SIZE as f64),
        mtm,
    );
    site.setTextColor(Some(&NSColor::secondaryLabelColor()));
    let identity = stack(
        NSUserInterfaceLayoutOrientation::Horizontal,
        metrics::SITE_INFO_GAP,
        mtm,
    );
    identity.addArrangedSubview(&icon);
    identity.addArrangedSubview(&site);

    let summary = NSTextField::wrappingLabelWithString(&NSString::new(), mtm);
    summary.setFont(Some(&NSFont::systemFontOfSize(
        metrics::MENU_FONT_SIZE as f64,
    )));
    summary.setSelectable(false);

    // Disabled until the Engine reports certificates; see Documentation/Omnibox.md.
    // SAFETY: a button without a target or action sends nothing.
    let certificate = unsafe {
        NSButton::buttonWithTitle_target_action(
            &NSString::from_str(VIEW_CERTIFICATE_LABEL),
            None,
            None,
            mtm,
        )
    };
    certificate.setBezelStyle(NSBezelStyle::Glass);
    certificate.setEnabled(false);

    let view = page(mtm);
    fill(&view, [&header, &identity, &summary]);
    view.addArrangedSubview(&certificate);
    (
        view,
        SecurityPage {
            icon,
            title,
            site,
            summary,
        },
        header,
    )
}

/// A page of the popover, padded and as wide as the GPUI popover.
fn page(mtm: MainThreadMarker) -> Retained<NSStackView> {
    let page = stack(
        NSUserInterfaceLayoutOrientation::Vertical,
        metrics::SITE_INFO_SECTION_GAP,
        mtm,
    );
    page.setAlignment(NSLayoutAttribute::Leading);
    let padding = metrics::SITE_INFO_PADDING as f64;
    page.setEdgeInsets(NSEdgeInsets {
        top: padding,
        left: padding,
        bottom: padding,
        right: padding,
    });
    page.widthAnchor()
        .constraintEqualToConstant(metrics::SITE_INFO_WIDTH as f64)
        .setActive(true);
    page
}

/// Adds `sections` to `page`, each across its full padded width.
fn fill<const N: usize>(page: &NSStackView, sections: [&NSView; N]) {
    let inset = 2.0 * metrics::SITE_INFO_PADDING as f64;
    for section in sections {
        page.addArrangedSubview(section);
        section
            .widthAnchor()
            .constraintEqualToAnchor_constant(&page.widthAnchor(), -inset)
            .setActive(true);
    }
}

fn stack(
    orientation: NSUserInterfaceLayoutOrientation,
    spacing: f32,
    mtm: MainThreadMarker,
) -> Retained<NSStackView> {
    let stack = NSStackView::new(mtm);
    stack.setOrientation(orientation);
    stack.setSpacing(spacing as f64);
    stack
}

/// Runs `action` on GPUI's executor, so entities update outside AppKit's
/// event callback.
fn in_app(app: &AsyncApp, action: impl FnMut(&mut App) + 'static) -> impl FnMut() + 'static {
    let app = app.clone();
    let action = Rc::new(RefCell::new(action));
    move || {
        let action = action.clone();
        app.spawn(async move |cx| cx.update(|cx| (action.borrow_mut())(cx)))
            .detach();
    }
}

/// A Liquid Glass button named `accessibility_label` for assistive
/// technologies and its tooltip.
fn glass_button(
    accessibility_label: &str,
    mut action: impl FnMut() + 'static,
    mtm: MainThreadMarker,
) -> (Retained<NSButton>, Retained<NativeControlActionTarget>) {
    let target = NativeControlActionTarget::new(mtm);
    // SAFETY: `target` registers the `activate:` selector.
    let button = unsafe {
        NSButton::buttonWithTitle_target_action(
            &NSString::new(),
            Some(&*target),
            Some(sel!(activate:)),
            mtm,
        )
    };
    button.setBezelStyle(NSBezelStyle::Glass);
    button.setImagePosition(NSCellImagePosition::ImageOnly);
    if !accessibility_label.is_empty() {
        let accessibility_label = NSString::from_str(accessibility_label);
        button.setToolTip(Some(&accessibility_label));
        button.setAccessibilityLabel(Some(&accessibility_label));
    }
    target.set_action(move |_| action());
    (button, target)
}

/// Makes `button` a round icon button as tall as the footer.
fn round(button: &NSButton, name: &str, description: &str) {
    button.setBezelStyle(NSBezelStyle::Circular);
    button.setImage(symbol(name, description).as_deref());
    set_size(button, metrics::SITE_INFO_FOOTER_HEIGHT);
}

/// A row with a round icon, a title and a quieter detail, dimmed while the
/// Engine cannot open it.
fn detail_row(detail: SiteDetail, mtm: MainThreadMarker) -> Retained<NSStackView> {
    let size = metrics::SITE_INFO_AVATAR_SIZE as f64;
    let avatar = NSView::new(mtm);
    avatar.setWantsLayer(true);
    if let Some(layer) = avatar.layer() {
        layer.setCornerRadius(size / 2.0);
        layer.setBackgroundColor(Some(&NSColor::quaternaryLabelColor().CGColor()));
    }
    set_size(&avatar, metrics::SITE_INFO_AVATAR_SIZE);
    let image = NSImageView::new(mtm);
    image.setImage(symbol(detail.symbol(), detail.title()).as_deref());
    image.setContentTintColor(Some(&NSColor::labelColor()));
    image.setFrame(NSRect::new(NSPoint::ZERO, NSSize::new(size, size)));
    avatar.addSubview(&image);

    let title = label(
        detail.title(),
        NSFont::systemFontOfSize(metrics::MENU_FONT_SIZE as f64),
        mtm,
    );
    let subtitle = label(
        detail.detail(),
        NSFont::systemFontOfSize(metrics::SITE_INFO_DETAIL_FONT_SIZE as f64),
        mtm,
    );
    subtitle.setTextColor(Some(&NSColor::secondaryLabelColor()));
    let text = stack(NSUserInterfaceLayoutOrientation::Vertical, 0.0, mtm);
    text.setAlignment(NSLayoutAttribute::Leading);
    text.addArrangedSubview(&title);
    text.addArrangedSubview(&subtitle);

    let row = stack(
        NSUserInterfaceLayoutOrientation::Horizontal,
        metrics::SITE_INFO_GAP * 1.5,
        mtm,
    );
    row.addArrangedSubview(&avatar);
    row.addArrangedSubview(&text);
    row.setAlphaValue(
        crate::platform::theme_config::theme()
            .opacity
            .disabled_control as f64,
    );
    row.setAccessibilityLabel(Some(&NSString::from_str(detail.title())));
    row
}

fn label(text: &str, font: Retained<NSFont>, mtm: MainThreadMarker) -> Retained<NSTextField> {
    let label = NSTextField::labelWithString(&NSString::from_str(text), mtm);
    label.setFont(Some(&font));
    label.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
    label.setContentCompressionResistancePriority_forOrientation(
        1.0,
        NSLayoutConstraintOrientation::Horizontal,
    );
    label
}

fn heading_font() -> Retained<NSFont> {
    NSFont::systemFontOfSize_weight(
        metrics::SITE_INFO_HEADING_FONT_SIZE as f64,
        // SAFETY: AppKit's font weight constants are immutable statics.
        unsafe { NSFontWeightSemibold },
    )
}

fn symbol(name: &str, description: &str) -> Option<Retained<NSImage>> {
    let image = NSImage::imageWithSystemSymbolName_accessibilityDescription(
        &NSString::from_str(name),
        Some(&NSString::from_str(description)),
    )?;
    let configuration = NSImageSymbolConfiguration::configurationWithPointSize_weight(
        metrics::SITE_INFO_TILE_ICON_SIZE as f64,
        // SAFETY: AppKit's font weight constants are immutable statics.
        unsafe { NSFontWeightRegular },
    );
    image.imageWithSymbolConfiguration(&configuration)
}

fn set_height(view: &NSView, height: f32) {
    view.heightAnchor()
        .constraintEqualToConstant(height as f64)
        .setActive(true);
}

fn set_size(view: &NSView, size: f32) {
    set_height(view, size);
    view.widthAnchor()
        .constraintEqualToConstant(size as f64)
        .setActive(true);
}
