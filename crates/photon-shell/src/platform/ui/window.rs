//! GPUI window bootstrap and top-level browser layout.

use gpui::{App, Context, Entity, KeyBinding, QuitMode, Render, Window, div, prelude::*, px};
use gpui_elements::editable_text::actions::{DEFAULT_INPUT_CONTEXT, default_bindings};
use gpui_platform::application;
use std::time::Duration;

use super::super::engine::UiWake;
use super::super::window_settings;
use super::PhotonWebView;
use super::omnibox::{FocusOmnibox, Omnibox};
use super::theme::metrics;
use super::titlebar::titlebar;

struct BrowserWindow {
    webview: Entity<PhotonWebView>,
    omnibox: Entity<Omnibox>,
}

impl Render for BrowserWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .on_action(cx.listener(|this, _: &FocusOmnibox, window, cx| {
                this.omnibox
                    .update(cx, |omnibox, cx| omnibox.focus(window, cx));
            }))
            .child(titlebar(self.omnibox.clone()))
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .px(px(metrics::PAGE_INSET))
                    .pb(px(metrics::PAGE_INSET))
                    .child(self.webview.clone()),
            )
    }
}

pub fn run() {
    application().run(|cx: &mut App| {
        // GPUI keeps macOS apps alive after their last window closes by default. Photon
        // has one browser window and owns an Engine runtime, so closing it must quit the
        // app and run the registered Engine/GPU shutdown path.
        cx.set_quit_mode(QuitMode::LastWindowClosed);
        cx.bind_keys(default_bindings().as_keybindings(Some(DEFAULT_INPUT_CONTEXT)));
        cx.bind_keys([KeyBinding::new("secondary-l", FocusOmnibox, None)]);
        let webview = create_webview(cx);
        quit_after_env_timeout(cx);
        cx.activate(true);
        cx.open_window(window_settings::options(cx), |window, cx| {
            window.set_window_title("Photon");
            webview.update(cx, |view, cx| {
                window.focus(&view.focus_handle, cx);
                view.track_engine_focus(window, cx);
            });
            let omnibox = cx.new(|cx| Omnibox::new(webview.clone(), window, cx));
            cx.new(|_| BrowserWindow { webview, omnibox })
        })
        .expect("open GPUI-CE Photon window");
    });
}

/// Starts the Engine-backed WebView, wired to redraw on Engine frames and to drain
/// GPU work before the app quits.
fn create_webview(cx: &mut App) -> Entity<PhotonWebView> {
    let webview = cx.new(|cx| {
        let mut webview = PhotonWebView::new(cx)
            .unwrap_or_else(|error| panic!("could not start direct PhotonWebView: {error:#}"));
        let gpu_activity = webview.session.gpu_activity.clone();
        webview._quit_subscription = Some(cx.on_app_quit(
            move |view: &mut PhotonWebView, _cx: &mut Context<'_, PhotonWebView>| {
                view.prepare_shutdown();
                gpu_activity.wait_until_idle();
                view.session.finish_shutdown();
                async {}
            },
        ));
        webview
    });
    let wake = UiWake {
        app: cx.to_async(),
        webview: webview.downgrade(),
    };
    webview.update(cx, |view, _| view.session.set_ui_wake(wake));
    webview
}

/// Quits after PHOTON_SHUTDOWN_AFTER_SECONDS, for timed shutdown checks.
fn quit_after_env_timeout(cx: &mut App) {
    let Some(seconds) = std::env::var("PHOTON_SHUTDOWN_AFTER_SECONDS")
        .ok()
        .and_then(|seconds| seconds.parse::<u64>().ok())
        .filter(|seconds| *seconds > 0)
    else {
        return;
    };
    let timer = cx.background_executor().timer(Duration::from_secs(seconds));
    cx.spawn(async move |cx| {
        timer.await;
        cx.update(|cx| cx.quit());
    })
    .detach();
}
