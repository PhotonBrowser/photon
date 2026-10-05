//! GPUI window bootstrap and top-level browser layout.

use gpui::{
    Animation, AnimationExt, App, Context, Entity, QuitMode, Render, Subscription, Transformation,
    Window, div, prelude::*, px, radians, rgb, svg,
};
use gpui_platform::application;
use std::time::Duration;

use super::super::engine::UiWake;
use super::super::window_settings;
use super::PhotonWebView;
use super::theme::metrics;

struct BrowserWindow {
    webview: Entity<PhotonWebView>,
    _activation_subscription: Option<Subscription>,
}

impl Render for BrowserWindow {
    fn render(&mut self, _: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let loading = self.webview.read(cx).loading;
        div()
            .flex()
            .flex_col()
            .size_full()
            .child(
                div()
                    .w_full()
                    .h(px(metrics::TITLEBAR_HEIGHT))
                    .flex_shrink_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .when(loading, |titlebar| {
                        titlebar.child(
                            div()
                                .flex()
                                .items_center()
                                .gap(px(6.0))
                                .text_size(px(11.0))
                                .text_color(rgb(0x5f6b76))
                                .child(loading_spinner())
                                .child("Loading"),
                        )
                    }),
            )
            .child(
                div()
                    .flex_1()
                    .w_full()
                    .p(px(metrics::PAGE_INSET))
                    .child(self.webview.clone()),
            )
    }
}

fn loading_spinner() -> impl IntoElement {
    svg()
        .data(
            br##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><path d="M12 3a9 9 0 1 0 9 9" fill="none" stroke="#477d99" stroke-width="3" stroke-linecap="round"/></svg>"##,
        )
        .size(px(14.0))
        .with_animation(
            "page-loading-spinner",
            Animation::new(Duration::from_millis(900))
                .repeat_synced()
                .with_max_fps(30.0),
            |spinner, phase| {
                spinner.with_transformation(Transformation::rotate(radians(
                    phase * std::f32::consts::TAU,
                )))
            },
        )
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
            let active = window.is_window_active();
            let _ = webview.update(cx, |view, _| view.session.set_focus(active));
            cx.new(|cx| {
                let activation_subscription = cx.observe_window_activation(
                    window,
                    |this: &mut BrowserWindow, window: &mut Window, cx: &mut Context<BrowserWindow>| {
                        let active = window.is_window_active();
                        let _ = this
                            .webview
                            .update(cx, |view, _| view.session.set_focus(active));
                    },
                );
                BrowserWindow {
                    webview,
                    _activation_subscription: Some(activation_subscription),
                }
            })
        })
        .expect("open GPUI-CE Photon window");
    });
}
