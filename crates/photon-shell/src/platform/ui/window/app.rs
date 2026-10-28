//! App startup: key bindings, the Engine runtime and the first window.

use crate::platform::shortcuts::{NewWindow, browser_shortcuts};
#[cfg(all(
    not(feature = "ui-only"),
    any(target_os = "linux", target_os = "windows")
))]
use gpui::AsyncApp;
use gpui::{App, QuitMode};
use gpui_elements::editable_text::actions::{DEFAULT_INPUT_CONTEXT, default_bindings};
use gpui_platform::application;
use photon_core::{BrowserSettings, Command, History, Session};
#[cfg(all(target_os = "macos", not(feature = "ui-only")))]
use std::cell::RefCell;
use std::rc::Rc;
use std::time::Duration;
#[cfg(target_os = "macos")]
use std::time::Instant;

use super::super::super::app_update::update_app;
use super::super::super::engine::EngineRuntime;
use super::super::history::load_browsing_history;
use super::super::settings::{Settings, load_settings};
#[cfg(unix)]
use super::automation::AutomationLaunch;
use super::menu::RunMenuCommand;
#[cfg(not(feature = "ui-only"))]
use super::open_urls::{open_received_urls, receive_opened_urls};
use super::page_menu::ActivatePageMenuItem;
use super::session::load_session;
#[cfg(all(target_os = "macos", not(feature = "ui-only")))]
use super::session::reopen_session;
#[cfg(not(feature = "ui-only"))]
use super::session::restore_session;
use super::{BrowserWindow, BrowserWindowRegistry, open_browser_window};
use photon_storage::Profile;

struct LoadedProfileState {
    profile: Option<Profile>,
    settings: BrowserSettings,
    settings_revision: Option<u64>,
    history: History,
    session: Session,
}

pub fn run() {
    #[cfg(unix)]
    let automation_launch = match AutomationLaunch::from_process_arguments() {
        Ok(launch) => launch,
        Err(error) => {
            eprintln!("Photon automation startup: {error}");
            std::process::exit(2);
        }
    };
    #[cfg(unix)]
    let automation_mode = automation_launch.is_some();
    #[cfg(windows)]
    let automation_mode = {
        let requested = std::env::args_os().any(|argument| {
            let argument = argument.to_string_lossy();
            argument == "--automation-socket" || argument.starts_with("--automation-socket=")
        });
        if requested {
            eprintln!("Photon automation sockets are not supported on Windows");
            std::process::exit(2);
        }
        false
    };
    let application = application();
    #[cfg(all(target_os = "macos", not(feature = "ui-only")))]
    let reopen_runtime = Rc::new(RefCell::new(None));
    #[cfg(all(target_os = "macos", not(feature = "ui-only")))]
    {
        let runtime = reopen_runtime.clone();
        application.on_reopen(move |cx| {
            if cx.windows().is_empty()
                && let Some(runtime) = runtime.borrow().as_ref()
            {
                reopen_session(runtime, cx);
            }
        });
    }
    #[cfg(not(feature = "ui-only"))]
    let opened_urls = receive_opened_urls(&application);
    application.run(move |cx: &mut App| {
        cx.set_global(BrowserWindowRegistry::default());
        // Keep the Engine runtime warm for its configured idle period after a
        // normal last-window close. Automation still exits with its last window.
        #[cfg(target_os = "macos")]
        if automation_mode {
            cx.set_quit_mode(QuitMode::LastWindowClosed);
        } else {
            cx.set_quit_mode(QuitMode::Explicit);
            keep_engine_warm_briefly(cx);
        }
        #[cfg(not(target_os = "macos"))]
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
        quit_after_env_timeout(cx);
        crate::platform::app_menus::install_app_menus(cx);
        let profile_state = cx
            .background_executor()
            .spawn(async move { load_profile_state() });
        cx.spawn(async move |cx| {
            let loaded = profile_state.await;
            let _ = update_app(cx, move |cx| {
                let LoadedProfileState {
                    profile,
                    settings,
                    settings_revision,
                    history,
                    session,
                } = loaded;
                load_settings(profile.clone(), settings, settings_revision, cx);
                load_browsing_history(profile.clone(), history, cx);
                #[cfg(feature = "ui-only")]
                let _ = load_session(None, session, cx);
                #[cfg(not(feature = "ui-only"))]
                let saved_session = load_session(profile.clone(), session, cx);

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
                #[cfg(all(
                    not(feature = "ui-only"),
                    any(target_os = "linux", target_os = "windows")
                ))]
                start_engine_pump(&runtime, cx.to_async());
                follow_settings(&runtime, cx);
                super::super::downloads::follow_downloads(&runtime, cx);
                #[cfg(not(feature = "ui-only"))]
                announce_service_restarts(&runtime, cx);

                let new_window_runtime = runtime.clone();
                #[cfg(all(target_os = "macos", not(feature = "ui-only")))]
                if !automation_mode {
                    *reopen_runtime.borrow_mut() = Some(runtime.clone());
                }
                cx.on_action(move |_: &NewWindow, cx| {
                    cx.activate(true);
                    if let Some(window) = cx
                        .active_window()
                        .and_then(|window| window.downcast::<BrowserWindow>())
                    {
                        cx.defer(move |cx| {
                            window
                                .update(cx, |browser, window, cx| {
                                    browser.execute_command(Command::NewWindow, window, cx);
                                })
                                .ok();
                        });
                    } else if let Err(error) = open_browser_window(
                        new_window_runtime.clone(),
                        launch_address(),
                        false,
                        false,
                        cx,
                    ) {
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

                #[cfg(not(feature = "ui-only"))]
                open_received_urls(opened_urls, runtime.clone(), cx);
                cx.activate(true);
                #[cfg(feature = "ui-only")]
                open_browser_window(
                    runtime,
                    Some(photon_core::internal_page_url("newtab")),
                    false,
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
                        open_browser_window(runtime, address, true, false, cx)
                            .expect("open GPUI-CE Photon window");
                    }
                }
                #[cfg(unix)]
                if let Some(launch) = automation_launch {
                    super::automation::install(launch, cx).unwrap_or_else(|error| {
                        panic!("could not start Photon automation server: {error}")
                    });
                }
            })
            .await;
        })
        .detach();
    });
}

/// Pumps Engine events on platforms without the Apple run-loop callback path.
#[cfg(all(
    not(feature = "ui-only"),
    any(target_os = "linux", target_os = "windows")
))]
fn start_engine_pump(runtime: &Rc<EngineRuntime>, app: AsyncApp) {
    let runtime = Rc::downgrade(runtime);
    app.spawn(async move |cx| {
        loop {
            cx.background_executor()
                .timer(Duration::from_millis(8))
                .await;
            let runtime = runtime.clone();
            let pumped = update_app(cx, move |_| {
                let Some(runtime) = runtime.upgrade() else {
                    return false;
                };
                runtime.pump();
                true
            })
            .await;
            if pumped != Some(true) {
                break;
            }
        }
    })
    .detach();
}

/// Retains the process and shared Engine runtime for a short idle window so a
/// newly opened browser window can reuse them. Closing a window destroys its
/// page views; after the grace period, quit if no window has reopened.
#[cfg(target_os = "macos")]
fn keep_engine_warm_briefly(cx: &mut App) {
    use crate::platform::app_update::update_app;
    use std::cell::Cell;

    let idle_deadline = Rc::new(Cell::new(None::<Instant>));
    let deadline_for_close = idle_deadline.clone();
    let app = cx.to_async();
    cx.on_window_closed(move |cx, _| {
        // Only the last window closing lets the app go; the others keep it.
        if !cx.windows().is_empty() {
            return;
        }
        let timeout = Duration::from_secs(u64::from(Settings::get(cx).engine_idle_timeout_seconds));
        if timeout.is_zero() {
            cx.quit();
            return;
        }
        let deadline = Instant::now() + timeout;
        deadline_for_close.set(Some(deadline));
        let deadline_for_timer = deadline_for_close.clone();
        let app = app.clone();
        app.spawn(async move |cx| {
            cx.background_executor().timer(timeout).await;
            let _ = update_app(cx, move |cx| {
                if deadline_for_timer.get() == Some(deadline) && cx.windows().is_empty() {
                    cx.quit();
                }
            })
            .await;
        })
        .detach();
    })
    .detach();
}

fn load_profile_state() -> LoadedProfileState {
    let profile = open_profile();
    let (settings, settings_revision) = profile
        .as_ref()
        .map(Profile::load_settings_with_revision)
        .unwrap_or_default();
    let history = profile
        .as_ref()
        .map(Profile::load_history)
        .unwrap_or_default();
    #[cfg(not(feature = "ui-only"))]
    let session = if settings.restore_session {
        profile
            .as_ref()
            .map(Profile::load_session)
            .unwrap_or_default()
    } else {
        Session::default()
    };
    #[cfg(feature = "ui-only")]
    let session = Session::default();
    LoadedProfileState {
        profile,
        settings,
        settings_revision,
        history,
        session,
    }
}

/// Applies the settings that belong to the whole app, now and whenever they
/// change. Page/menu appearance is resolved separately from native window tint.
fn follow_settings(runtime: &Rc<EngineRuntime>, cx: &mut App) {
    let apply = |runtime: &EngineRuntime, cx: &mut App| {
        cx.set_window_appearance(None);
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
