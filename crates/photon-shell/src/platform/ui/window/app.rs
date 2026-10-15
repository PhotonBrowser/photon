//! App startup: key bindings, the Engine runtime and the first window.

use gpui::{App, QuitMode};
use gpui_elements::editable_text::actions::{DEFAULT_INPUT_CONTEXT, default_bindings};
use gpui_platform::application;
use photon_shortcuts::{NewWindow, browser_shortcuts};
use std::rc::Rc;
use std::time::Duration;

#[cfg(not(feature = "ui-only"))]
use super::super::super::app_update::update_app;
use super::super::super::engine::EngineRuntime;
use super::super::history::load_browsing_history;
use super::super::install_form_controls;
use super::super::settings::{Settings, load_settings};
use super::automation::AutomationLaunch;
use super::menu::RunMenuCommand;
#[cfg(not(feature = "ui-only"))]
use super::open_urls::{open_received_urls, receive_opened_urls};
use super::page_menu::ActivatePageMenuItem;
use super::session::load_session;
#[cfg(not(feature = "ui-only"))]
use super::session::restore_session;
use super::{BrowserWindow, BrowserWindowRegistry, open_browser_window};
use photon_storage::Profile;

pub fn run() {
    let automation_launch = match AutomationLaunch::from_process_arguments() {
        Ok(launch) => launch,
        Err(error) => {
            eprintln!("Photon automation startup: {error}");
            std::process::exit(2);
        }
    };
    let application = application();
    #[cfg(not(feature = "ui-only"))]
    let opened_urls = receive_opened_urls(&application);
    let automation_mode = automation_launch.is_some();
    application.run(move |cx: &mut App| {
        cx.set_global(BrowserWindowRegistry::default());
        // Quit when the last browser window closes so the shell can stop its
        // Engine runtime and release native resources.
        cx.set_quit_mode(QuitMode::LastWindowClosed);
        if automation_mode {
            cx.on_app_quit(|cx| {
                crate::platform::automation_lifecycle_trace(format_args!(
                    "app_quit open_windows={}",
                    cx.windows().len()
                ));
                async {}
            })
            .detach();
        }
        cx.bind_keys(default_bindings().as_keybindings(Some(DEFAULT_INPUT_CONTEXT)));
        // Keep browser actions and their default bindings in the shortcuts crate.
        cx.bind_keys(browser_shortcuts());
        crate::platform::motion_observer::follow(cx);
        let profile = open_profile();
        load_settings(profile.clone(), cx);
        #[cfg(feature = "ui-only")]
        let runtime = Rc::new(EngineRuntime::without_engine());
        #[cfg(not(feature = "ui-only"))]
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
        #[cfg(feature = "ui-only")]
        load_session(None, cx);
        #[cfg(not(feature = "ui-only"))]
        let saved_session = load_session(profile.clone(), cx);
        load_browsing_history(profile, cx);
        #[cfg(not(feature = "ui-only"))]
        announce_service_restarts(&runtime, cx);
        quit_after_env_timeout(cx);
        crate::platform::app_menus::install_app_menus(cx);
        let new_window_runtime = runtime.clone();
        cx.on_action(move |_: &NewWindow, cx| {
            cx.activate(true);
            if let Err(error) =
                open_browser_window(new_window_runtime.clone(), launch_address(), false, cx)
            {
                crate::platform::trace(format_args!("new window: {error:#}"));
            }
        });
        cx.on_action(|item: &RunMenuCommand, cx| {
            let command = item.clone();
            let Some(window) = cx
                .active_window()
                .and_then(|window| window.downcast::<BrowserWindow>())
            else {
                return;
            };
            // Native menu actions are validated independently of the focused
            // view, then dispatched through the app. Defer window access until
            // that dispatch has released its current window update.
            cx.defer(move |cx| {
                window
                    .update(cx, |browser, window, cx| {
                        browser.run_menu_command(&command, window, cx);
                    })
                    .ok();
            });
        });
        cx.on_action(|item: &ActivatePageMenuItem, cx| {
            let item = item.clone();
            let Some(window) = cx
                .active_window()
                .and_then(|window| window.downcast::<BrowserWindow>())
            else {
                return;
            };
            cx.defer(move |cx| {
                window
                    .update(cx, |browser, _, cx| {
                        browser.activate_page_menu_item(&item, cx);
                    })
                    .ok();
            });
        });
        install_form_controls(cx);
        #[cfg(not(feature = "ui-only"))]
        open_received_urls(opened_urls, runtime.clone(), cx);
        cx.activate(true);
        #[cfg(feature = "ui-only")]
        open_browser_window(
            runtime,
            Some(photon_core::internal_page_url("newtab")),
            false,
            cx,
        )
        .expect("open UI-only Photon window");
        #[cfg(not(feature = "ui-only"))]
        {
            // An address to open starts afresh; otherwise last time's windows
            // come back, or a new one opens.
            let address = initial_address();
            if address.is_some() || !restore_session(&saved_session, &runtime, cx) {
                open_browser_window(runtime, address, true, cx)
                    .expect("open GPUI-CE Photon window");
            }
        }
        if let Some(launch) = automation_launch {
            super::automation::install(launch, cx).unwrap_or_else(|error| {
                panic!("could not start Photon automation server: {error}")
            });
        }
    });
}

/// Applies the settings that belong to the whole app, now and whenever they
/// change: the window appearance and the pop-up policy.
fn follow_settings(runtime: &Rc<EngineRuntime>, cx: &mut App) {
    let apply = |runtime: &EngineRuntime, cx: &mut App| {
        cx.set_window_appearance(Settings::chosen_appearance(cx));
        runtime.set_popup_policy(Settings::get(cx).popup_policy);
        runtime.set_elastic_scroll(Settings::get(cx).elastic_scroll);
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

/// The address a newly opened window starts at.
pub(super) fn launch_address() -> Option<String> {
    #[cfg(feature = "ui-only")]
    {
        Some(photon_core::internal_page_url("newtab"))
    }
    #[cfg(not(feature = "ui-only"))]
    {
        initial_address()
    }
}

/// The address to open at startup, from PHOTON_URL.
#[cfg(not(feature = "ui-only"))]
pub(super) fn initial_address() -> Option<String> {
    std::env::var("PHOTON_URL")
        .ok()
        .map(|address| address.trim().to_owned())
        .filter(|address| !address.is_empty())
}

/// Shows Engine service stops and restarts in every window.
#[cfg(not(feature = "ui-only"))]
fn announce_service_restarts(runtime: &EngineRuntime, cx: &mut App) {
    let app = cx.to_async();
    runtime.on_service_change(move |_, restarted| {
        app.spawn(async move |cx| {
            update_app(cx, |cx| {
                for window in cx.windows() {
                    if let Some(window) = window.downcast::<BrowserWindow>() {
                        window
                            .update(cx, |this, _, cx| this.engine_service_changed(restarted, cx))
                            .ok();
                    }
                }
            })
            .await;
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
