//! App startup: key bindings, the Engine runtime and the first window.

use gpui::{App, Global, QuitMode};
use gpui_elements::editable_text::actions::{DEFAULT_INPUT_CONTEXT, default_bindings};
use gpui_platform::application;
use photon_shortcuts::browser_shortcuts;
use std::rc::Rc;
use std::time::Duration;

use super::super::super::engine::EngineRuntime;
use super::super::super::motion_observer::ReducedMotionObserver;
use super::super::theme::ThemePreference;
use super::open_browser_window;

/// Keeps the system reduced-motion preference flowing into GPUI.
struct ReducedMotion {
    _observer: Option<ReducedMotionObserver>,
}

impl Global for ReducedMotion {}

pub fn run() {
    application().run(|cx: &mut App| {
        // GPUI keeps macOS apps alive after their last window closes by default. Photon
        // has one browser window and owns an Engine runtime, so closing it must quit the
        // app and run the registered Engine/GPU shutdown path.
        cx.set_quit_mode(QuitMode::LastWindowClosed);
        cx.bind_keys(default_bindings().as_keybindings(Some(DEFAULT_INPUT_CONTEXT)));
        // Keep browser actions and their default bindings in the shortcuts crate.
        cx.bind_keys(browser_shortcuts());
        follow_reduced_motion(cx);
        let runtime = Rc::new(
            EngineRuntime::create()
                .unwrap_or_else(|error| panic!("could not start Photon Engine: {error:#}")),
        );
        quit_after_env_timeout(cx);
        cx.activate(true);
        open_browser_window(runtime, initial_address(), ThemePreference::default(), cx)
            .expect("open GPUI-CE Photon window");
    });
}

/// The address to open at startup, from PHOTON_URL.
pub(super) fn initial_address() -> Option<String> {
    std::env::var("PHOTON_URL")
        .ok()
        .map(|address| address.trim().to_owned())
        .filter(|address| !address.is_empty())
}

/// Keeps GPUI's reduce-motion flag in step with the system preference.
fn follow_reduced_motion(cx: &mut App) {
    let app = cx.to_async();
    let observer = ReducedMotionObserver::new(move |reduce_motion| {
        app.spawn(async move |cx| cx.update(|cx| cx.set_reduce_motion(reduce_motion)))
            .detach();
    });
    cx.set_global(ReducedMotion {
        _observer: observer,
    });
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
