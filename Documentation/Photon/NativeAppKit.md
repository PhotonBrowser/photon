# Native AppKit elements in GPUI

`photon-shell` exposes a small macOS bridge for placing real AppKit views in a
GPUI layout. AppKit owns the view's drawing, input, and accessibility. GPUI
reserves its size, updates its frame after layout, and attaches or detaches it
with the element's lifetime.

## AppKit buttons

Create a button once in a retained GPUI view, then clone it into the element
tree during render:

```rust
let app = cx.to_async();
let select_button = AppKitButton::symbol("cursorarrow", "Select")
    .expect("the system provides the cursor symbol")
    .on_click(app, |_cx| {
        // Handle the native AppKit action in GPUI's foreground context.
    });
```

Render it alongside ordinary GPUI elements. The native control uses its
AppKit intrinsic size unless a parent supplies dimensions:

```rust
div()
    .flex()
    .gap(px(8.))
    .child(select_button.clone().w(px(36.)).h(px(36.)))
```

`AppKitButton::symbol(symbol, accessibility_label)` creates an SF Symbol
button. `AppKitButton::title(label)` creates a text button. Buttons default to
`AppKitButtonStyle::LiquidGlass`: on macOS 26 and later, AppKit embeds the
borderless button in an interactive
[`NSGlassEffectView`](https://developer.apple.com/documentation/appkit/nsglasseffectview)
and uses the button's intrinsic size for GPUI layout. It falls back to the
native toolbar bezel on earlier macOS. Use
`.appearance(AppKitButtonStyle::Toolbar)` or
`.appearance(AppKitButtonStyle::Borderless)` to choose another system
appearance. `.enabled(bool)` controls the native button's enabled state,
`.tooltip(text)` sets its native help text, and `.on_click(app, handler)`
schedules the action on GPUI's foreground executor.

## Native tab bars

`AppKitTabBar` wraps an `NSSegmentedControl` in single-selection mode. On
macOS 27+, it uses AppKit's native tabs role for the tab appearance and
VoiceOver semantics. On macOS 26, it hosts the segmented control in an
interactive `NSGlassEffectView`. Its selection callback lets the GPUI view
choose which content to render:

```rust
let tabs = AppKitTabBar::new(&["Overview", "Activity", "Settings"])
    .expect("AppKit creates a non-empty tab bar")
    .on_change(cx.to_async(), move |index, cx| {
        page.update(cx, |page, cx| {
            page.selected_tab = index;
            cx.notify();
        });
    });

div().child(tabs.clone().w_full())
```

Use `selected_index()` to read AppKit's current selection and
`set_selected_index(index)` when GPUI changes it programmatically.

## Hosting an existing `NSView`

Wrap an owned AppKit view pointer (`NonNull<c_void>` with a +1 retain count),
then use its GPUI element:

```rust
let view = unsafe { AppKitView::from_raw_owned(owned_ns_view) }
    .expect("AppKit views are created on the main thread");

div().child(view.element().w(px(240.)).h(px(120.)))
```

`from_raw_owned` is unsafe because the pointer must be a live `NSView`
subclass with a +1 retain count. It consumes that ownership. Keep native view
creation and use on AppKit's main thread. A single native view can only be
mounted once at a time; use a separate `NSView` instance for each layout slot.

## Composition limits

The host updates the native view's bounds during GPUI prepaint. It automatically
detaches the view when that element leaves the tree and when its GPUI window
closes. Layout, intrinsic sizing, and the element's own visibility and opacity
follow GPUI. AppKit continues to draw the control and dispatch its native
events.

Native child views are composited above the GPUI-rendered surface. GPUI masks,
clipping, ancestor opacity groups, transforms, paint order, and overlays cannot
cover or clip them. Keep native controls in non-overlapping slots above content;
use native AppKit controls for interactions that must appear above a hosted
native view. This adapter is macOS-only; Linux and Windows continue to use the
shared GPUI controls.
