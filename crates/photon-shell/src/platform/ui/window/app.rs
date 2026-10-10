//! App startup: key bindings, the Engine runtime and the first window.

use gpui::{App, Global, QuitMode};
use gpui_elements::editable_text::actions::{DEFAULT_INPUT_CONTEXT, default_bindings};
use gpui_platform::application;
use photon_shortcuts::browser_shortcuts;
use std::rc::Rc;
use std::time::Duration;

use super::super::super::engine::EngineRuntime;
use super::super::super::motion_observer::ReducedMotionObserver;
use super::super::history::load_browsing_history;
use super::super::settings::{Settings, load_settings};
use super::{BrowserWindow, open_browser_window};
use photon_storage::Profile;

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
        let profile = open_profile();
        load_settings(profile.clone(), cx);
        let runtime = Rc::new(
            EngineRuntime::create(
                profile
                    .as_ref()
                    .map(|profile| profile.engine_dir())
                    .as_deref(),
            )
            .unwrap_or_else(|error| panic!("could not start Photon Engine: {error:#}")),
        );
        follow_settings(&runtime, cx);
        load_browsing_history(profile, cx);
        announce_service_restarts(&runtime, cx);
        quit_after_env_timeout(cx);
        cx.activate(true);
        open_browser_window(runtime, initial_address(), true, cx)
            .expect("open GPUI-CE Photon window");
    });
}

/// Applies the settings that belong to the whole app, now and whenever they
/// change: the window appearance and the pop-up policy.
fn follow_settings(runtime: &Rc<EngineRuntime>, cx: &mut App) {
    let apply = |runtime: &EngineRuntime, cx: &mut App| {
        cx.set_window_appearance(Settings::chosen_appearance(cx));
        runtime.set_popup_policy(Settings::get(cx).popup_policy);
    };
    apply(runtime, cx);
    let runtime = runtime.clone();
    cx.observe_global::<Settings>(move |cx| apply(&runtime, cx))
        .detach();
}

/// The profile Photon keeps history and website data in, or `None` for a
/// temporary session that remembers nothing, as with PHOTON_TEMPORARY_PROFILE.
fn open_profile() -> Option<Profile> {
    if std::env::var_os("PHOTON_TEMPORARY_PROFILE").is_some() {
        return None;
    }
    Profile::open_default()
        .inspect_err(|error| eprintln!("Photon: using a temporary profile: {error}"))
        .ok()
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

/// Shows Engine service stops and restarts in every window.
fn announce_service_restarts(runtime: &EngineRuntime, cx: &mut App) {
    let app = cx.to_async();
    runtime.on_service_change(move |_, restarted| {
        app.spawn(async move |cx| {
            cx.update(|cx| {
                for window in cx.windows() {
                    if let Some(window) = window.downcast::<BrowserWindow>() {
                        window
                            .update(cx, |this, _, cx| this.engine_service_changed(restarted, cx))
                            .ok();
                    }
                }
            })
        })
        .detach();
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
