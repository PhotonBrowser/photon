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
use objc2_quartz_core::{CAMediaTiming, CATransition, kCATransitionFade};

use super::appkit::{NativeControlActionTarget, appkit_host_view};

mod views;
use crate::platform::metrics;
use crate::platform::site_info::{
    BACK_LABEL, CLOSE_LABEL, ConnectionStatus, SiteControl, SiteDetail, SiteInfo, TITLE,
    VIEW_CERTIFICATE_LABEL,
};
use views::*;

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
    /// Switches to `page`. While open, the popover animates to the page's
    /// size and cross-fades its content. The fade is a layer transition so
    /// glass controls never render at partial opacity, which turns them black.
    fn show(&self, page: &NSView) {
        // SAFETY: the page's superview is the popover's live content view,
        // read on the main thread.
        let container = unsafe { self.controller.view().superview() };
        if self.popover.isShown()
            && let Some(layer) = container.and_then(|view| view.layer())
        {
            let fade = CATransition::animation();
            // SAFETY: an immutable AppKit constant.
            fade.setType(unsafe { kCATransitionFade });
            fade.setDuration(metrics::SITE_INFO_PAGE_DURATION_MS as f64 / 1000.0);
            layer.addAnimation_forKey(&fade, None);
        }
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
        // Prefer the edge that sits visually below the anchor, so resizing
        // between pages never makes AppKit flip the popover to the other side.
        let (y, below) = if host.isFlipped() {
            (y, NSRectEdge::MaxY)
        } else {
            (host.bounds().size.height - y, NSRectEdge::MinY)
        };
        self.inner.pages.show(&self.inner.pages.site);
        self.inner.popover.showRelativeToRect_ofView_preferredEdge(
            NSRect::new(NSPoint::new(x, y), NSSize::new(1.0, 1.0)),
            host,
            below,
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
