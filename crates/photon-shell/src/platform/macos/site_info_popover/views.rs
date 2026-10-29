//! The views the site information popover is built from.

use super::*;

/// The connection's page: its heading, the site with its lock, and what the
/// connection means. Returns the page, its fields, and the header row the
/// back button joins.
pub(super) fn security_page(
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
pub(super) fn page(mtm: MainThreadMarker) -> Retained<NSStackView> {
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
pub(super) fn fill<const N: usize>(page: &NSStackView, sections: [&NSView; N]) {
    let inset = 2.0 * metrics::SITE_INFO_PADDING as f64;
    for section in sections {
        page.addArrangedSubview(section);
        section
            .widthAnchor()
            .constraintEqualToAnchor_constant(&page.widthAnchor(), -inset)
            .setActive(true);
    }
}

pub(super) fn stack(
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
pub(super) fn in_app(
    app: &AsyncApp,
    action: impl FnMut(&mut App) + 'static,
) -> impl FnMut() + 'static {
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
pub(super) fn glass_button(
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
pub(super) fn round(button: &NSButton, name: &str, description: &str) {
    button.setBezelStyle(NSBezelStyle::Circular);
    button.setImage(symbol(name, description).as_deref());
    set_size(button, metrics::SITE_INFO_FOOTER_HEIGHT);
}

/// A row with a round icon, a title and a quieter detail, dimmed while the
/// Engine cannot open it.
pub(super) fn detail_row(detail: SiteDetail, mtm: MainThreadMarker) -> Retained<NSStackView> {
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

pub(super) fn label(
    text: &str,
    font: Retained<NSFont>,
    mtm: MainThreadMarker,
) -> Retained<NSTextField> {
    let label = NSTextField::labelWithString(&NSString::from_str(text), mtm);
    label.setFont(Some(&font));
    label.setLineBreakMode(NSLineBreakMode::ByTruncatingTail);
    label.setContentCompressionResistancePriority_forOrientation(
        1.0,
        NSLayoutConstraintOrientation::Horizontal,
    );
    label
}

pub(super) fn heading_font() -> Retained<NSFont> {
    NSFont::systemFontOfSize_weight(
        metrics::SITE_INFO_HEADING_FONT_SIZE as f64,
        // SAFETY: AppKit's font weight constants are immutable statics.
        unsafe { NSFontWeightSemibold },
    )
}

pub(super) fn symbol(name: &str, description: &str) -> Option<Retained<NSImage>> {
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

pub(super) fn set_height(view: &NSView, height: f32) {
    view.heightAnchor()
        .constraintEqualToConstant(height as f64)
        .setActive(true);
}

pub(super) fn set_size(view: &NSView, size: f32) {
    set_height(view, size);
    view.widthAnchor()
        .constraintEqualToConstant(size as f64)
        .setActive(true);
}
