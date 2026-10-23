//! A page's form controls that open native UI: a `<select>` element's
//! dropdown, a native menu under the element, and a file input's open panel.
//! The Engine waits for an answer to each, so every way they close answers.

use gpui::{
    App, Context, Global, MenuItem, NoAction, PathPromptOptions, SharedString, WeakEntity, point,
    px,
};

use super::super::super::app_update::update_app;
use super::PhotonWebView;

/// An entry in a `<select>` element's dropdown.
pub(in crate::platform) enum SelectItem {
    Option {
        label: SharedString,
        id: u32,
        selected: bool,
        enabled: bool,
    },
    /// The label of the options after it, up to the next label.
    GroupLabel(SharedString),
    Separator,
}

/// A dropdown a `<select>` element asked for, opening at `position` in the
/// page, in logical pixels, and at least `minimum_width` wide.
pub(in crate::platform) struct SelectDropdown {
    pub(in crate::platform) position: (f32, f32),
    pub(in crate::platform) minimum_width: f32,
    pub(in crate::platform) items: Vec<SelectItem>,
}

/// Chooses an option in the open `<select>` dropdown.
#[derive(Clone, PartialEq, gpui::Action)]
#[action(namespace = photon, no_json)]
struct ChooseSelectOption {
    id: u32,
}

/// The page whose `<select>` dropdown is open. Only one native menu is open
/// at a time.
struct OpenSelectDropdown(WeakEntity<PhotonWebView>);

impl Global for OpenSelectDropdown {}

/// Identifies the one native file panel that can currently answer a page.
/// A late panel result is ignored after navigation or tab teardown.
struct OpenFilePicker {
    webview: WeakEntity<PhotonWebView>,
    request: u64,
}

impl Global for OpenFilePicker {}
static NEXT_FILE_PICKER_REQUEST: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(1);

/// Has a chosen option reach the page whose dropdown is open, wherever focus
/// is when the menu runs its action.
pub(in crate::platform) fn install_form_controls(cx: &mut App) {
    cx.on_action(|choice: &ChooseSelectOption, cx| {
        if cx.has_global::<OpenSelectDropdown>() {
            let OpenSelectDropdown(webview) = cx.remove_global();
            webview
                .update(cx, |view, _| {
                    view.session.select_dropdown_closed(Some(choice.id))
                })
                .ok();
        }
    });
}

impl PhotonWebView {
    /// Shows a `<select>` element's options as a native menu under it.
    pub(in crate::platform) fn show_select_dropdown(
        &mut self,
        dropdown: SelectDropdown,
        cx: &mut Context<Self>,
    ) {
        let Some(window) = self.window else {
            self.session.select_dropdown_closed(None);
            return;
        };
        let (x, y) = self.input.viewport_origin();
        let position = point(px(x + dropdown.position.0), px(y + dropdown.position.1));
        let menu = dropdown.items.into_iter().map(select_menu_item).collect();
        let shown = window.update(cx, |_, window, cx| {
            window.show_menu_at(menu, position, px(dropdown.minimum_width), cx)
        });
        let Ok(closed) = shown else {
            self.session.select_dropdown_closed(None);
            return;
        };
        let webview = cx.entity().downgrade();
        cx.set_global(OpenSelectDropdown(webview));
        cx.spawn(async move |this, cx| {
            closed.await.ok();
            // A chosen option has already answered; anything else dismissed it.
            update_app(cx, |cx| {
                if cx
                    .try_global::<OpenSelectDropdown>()
                    .is_some_and(|open| open.0 == this)
                {
                    cx.remove_global::<OpenSelectDropdown>();
                    this.update(cx, |view, _| view.session.select_dropdown_closed(None))
                        .ok();
                }
            })
            .await;
        })
        .detach();
    }

    /// Asks for files to give a file input, in the system's open panel.
    pub(in crate::platform) fn open_file_picker(
        &mut self,
        allow_multiple: bool,
        cx: &mut Context<Self>,
    ) {
        let request = NEXT_FILE_PICKER_REQUEST.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let webview = cx.entity().downgrade();
        cx.set_global(OpenFilePicker { webview, request });
        let chosen = cx.prompt_for_paths(PathPromptOptions {
            files: true,
            directories: false,
            multiple: allow_multiple,
            prompt: None,
        });
        cx.spawn(async move |this, cx| {
            let paths = chosen
                .await
                .ok()
                .and_then(Result::ok)
                .flatten()
                .unwrap_or_default();
            update_app(cx, |cx| {
                if cx
                    .try_global::<OpenFilePicker>()
                    .is_some_and(|open| open.request == request)
                {
                    cx.remove_global::<OpenFilePicker>();
                    this.update(cx, |view, _| view.session.file_picker_closed(&paths))
                        .ok();
                }
            })
            .await;
        })
        .detach();
    }

    pub(in crate::platform) fn cancel_file_picker(&mut self, cx: &mut Context<Self>) {
        if cx
            .try_global::<OpenFilePicker>()
            .is_some_and(|open| open.webview == cx.entity().downgrade())
        {
            cx.remove_global::<OpenFilePicker>();
            self.session.file_picker_closed(&[]);
        }
    }
}

/// A dropdown entry as a native menu item. A group's label is a disabled
/// item, as in a native pop-up button's menu.
fn select_menu_item(item: SelectItem) -> MenuItem {
    match item {
        SelectItem::Option {
            label,
            id,
            selected,
            enabled,
        } => MenuItem::Action {
            name: label,
            action: Box::new(ChooseSelectOption { id }),
            os_action: None,
            checked: selected,
            disabled: !enabled,
        },
        SelectItem::GroupLabel(label) => MenuItem::Action {
            name: label,
            action: Box::new(NoAction),
            os_action: None,
            checked: false,
            disabled: true,
        },
        SelectItem::Separator => MenuItem::separator(),
    }
}
