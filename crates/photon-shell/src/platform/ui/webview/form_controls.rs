//! Page form controls: GPUI `<select>` menus and the file input's open panel.
//! The Engine waits for an answer to each, so every way they close answers.

use gpui::{Context, Global, PathPromptOptions, SharedString, WeakEntity};

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

/// Identifies the one native file panel that can currently answer a page.
/// A late panel result is ignored after navigation or tab teardown.
struct OpenFilePicker {
    webview: WeakEntity<PhotonWebView>,
    request: u64,
}

impl Global for OpenFilePicker {}
static NEXT_FILE_PICKER_REQUEST: std::sync::atomic::AtomicU64 =
    std::sync::atomic::AtomicU64::new(1);

impl PhotonWebView {
    /// Asks the browser window to show a styled menu under this `<select>`.
    pub(in crate::platform) fn show_select_dropdown(
        &mut self,
        dropdown: SelectDropdown,
        cx: &mut Context<Self>,
    ) {
        if self.window.is_none() {
            self.session.select_dropdown_closed(None);
            return;
        }
        self.select_dropdown_request = Some(dropdown);
        cx.emit(super::WebViewEvent::SelectDropdownRequested);
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
