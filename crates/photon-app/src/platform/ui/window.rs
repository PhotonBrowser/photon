//! GPUI window bootstrap and top-level browser layout.

use gpui::{App, Context, Entity, QuitMode, Render, Window, div, prelude::*, px};
use gpui_platform::application;
use std::time::Duration;

use super::super::engine::UiWake;
use super::super::window_settings;
use super::super::window_settings::{TITLEBAR_HEIGHT, WEBVIEW_CORNER_RADIUS, WEBVIEW_INSET};
use super::PhotonWebView;

struct BrowserWindow {
    webview: Entity<PhotonWebView>,
}
impl Render for BrowserWindow {
    fn render(&mut self, _: &mut Window, _: &mut Context<Self>) -> impl IntoElement {
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(div().w_full().h(px(TITLEBAR_HEIGHT)).flex_shrink_0())
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .p(px(WEBVIEW_INSET))
                    .overflow_hidden()
                    .rounded(px(WEBVIEW_CORNER_RADIUS))
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
        let _ = webview.update(cx, |view, _| view.session.set_ui_wake(wake));

        if let Ok(seconds) = std::env::var("PHOTON_SHUTDOWN_AFTER_SECONDS")
            && let Ok(seconds) = seconds.parse::<u64>()
            && seconds > 0
        {
            let timer = cx.background_executor().timer(Duration::from_secs(seconds));
            cx.spawn(async move |cx| {
                timer.await;
                let _ = cx.update(|cx| cx.quit());
            })
            .detach();
        }
        cx.activate(true);
        cx.open_window(window_settings::options(cx), |window, cx| {
            window.set_window_title("Photon");
            cx.new(|_| BrowserWindow { webview })
        })
        .expect("open GPUI-CE Photon window");
    });
}
