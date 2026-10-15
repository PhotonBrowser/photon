//! The open windows and their tabs, kept in the profile as they change so
//! the browser reopens them as they were after a restart.
//!
//! Each window records its everyday tabs as it redraws; a change is saved a
//! moment later, off the main thread, and once more when the app quits.
//! Closing one of several windows forgets it; closing the last keeps it, as
//! that quits the browser.

#[cfg(not(feature = "ui-only"))]
use gpui::Window;
use gpui::{App, BorrowAppContext, Context, Global, Task};
use photon_core::{Session, SessionTab, WindowSession};
use photon_storage::{Profile, ProfileWriteTicket};
#[cfg(not(feature = "ui-only"))]
use std::rc::Rc;
use std::time::Duration;

#[cfg(not(feature = "ui-only"))]
use super::super::super::engine::EngineRuntime;
use super::super::super::trace;
#[cfg(not(feature = "ui-only"))]
use super::super::pages::find_page;
use super::super::settings::Settings;
use super::BrowserWindow;
use super::command_bar::tab_summary;
#[cfg(not(feature = "ui-only"))]
use super::content::create_webview;
#[cfg(not(feature = "ui-only"))]
use super::open_browser_window;
use crate::platform::app_update::update_app;

/// How long after a change the session is saved, so a burst saves once.
const SAVE_DELAY: Duration = Duration::from_secs(1);

#[derive(Default)]
struct BrowsingSession {
    /// Where the session is saved; `None` keeps it in memory only.
    profile: Option<Profile>,
    /// Each open window's tabs, by window, in the order they opened.
    windows: Vec<(u64, WindowSession)>,
    save: Option<Task<()>>,
}

impl Global for BrowsingSession {}

impl BrowsingSession {
    fn session(&self) -> Session {
        Session {
            windows: self
                .windows
                .iter()
                .map(|(_, window)| window.clone())
                .collect(),
        }
    }

    fn snapshot(&self) -> Option<(Profile, Session, ProfileWriteTicket)> {
        let profile = self.profile.clone()?;
        let ticket = profile.begin_session_write();
        Some((profile, self.session(), ticket))
    }

    /// Saves the session a moment from now, unless a save is waiting.
    fn save_soon(&mut self, cx: &mut App) {
        if self.profile.is_none() || self.save.is_some() {
            return;
        }
        self.save = Some(cx.spawn(async move |cx| {
            cx.background_executor().timer(SAVE_DELAY).await;
            let save = update_app(cx, |cx| {
                cx.update_global::<Self, _>(|session, _| {
                    session.save = None;
                    session.snapshot()
                })
            })
            .await
            .flatten();
            // Clearing `save` above dropped this task's handle, which
            // cancels it at its next await, so the write runs detached.
            if let Some((profile, session, ticket)) = save {
                cx.background_executor()
                    .spawn(async move { write_session(&profile, &session, ticket) })
                    .detach();
            }
        }));
    }
}

fn write_session(profile: &Profile, session: &Session, ticket: ProfileWriteTicket) {
    if let Err(error) = profile.save_session_if_current(ticket, session) {
        eprintln!("failed to save session: {error}");
    }
}

/// Keeps the session in `profile`, and returns the one saved last time when
/// the settings ask for it to be reopened.
pub(super) fn load_session(profile: Option<Profile>, cx: &mut App) -> Session {
    let saved = profile
        .as_ref()
        .filter(|_| Settings::get(cx).restore_session)
        .map(Profile::load_session)
        .unwrap_or_default();
    cx.set_global(BrowsingSession {
        profile,
        ..BrowsingSession::default()
    });
    cx.on_app_quit(|cx| {
        let save = cx.update_global::<BrowsingSession, _>(|session, _| {
            session.save = None;
            session.snapshot()
        });
        async move {
            if let Some((profile, session, ticket)) = save {
                write_session(&profile, &session, ticket);
            }
        }
    })
    .detach();
    saved
}

/// Reopens the windows of a saved session, the first with the pinned tabs
/// too, as the first window at launch has. Returns whether any opened.
#[cfg(not(feature = "ui-only"))]
pub(super) fn restore_session(
    session: &Session,
    runtime: &Rc<EngineRuntime>,
    cx: &mut App,
) -> bool {
    let mut opened = false;
    for saved in session
        .windows
        .iter()
        .filter(|window| !window.tabs.is_empty())
    {
        let window = match open_browser_window(runtime.clone(), None, !opened, cx) {
            Ok(window) => window,
            Err(error) => {
                trace(format_args!("restoring window: {error:#}"));
                continue;
            }
        };
        window
            .update(cx, |browser, window, cx| {
                browser.restore_tabs(saved, window, cx)
            })
            .ok();
        opened = true;
    }
    opened
}

impl BrowserWindow {
    /// Records this window's tabs in the session when they have changed.
    pub(super) fn record_session(&self, cx: &mut Context<Self>) {
        let state = self.session_state(cx);
        self.record_session_state(state, cx);
    }

    /// Records `state` as this window's tabs, saving it when it changed.
    pub(super) fn record_session_state(&self, state: WindowSession, cx: &mut Context<Self>) {
        let key = cx.entity_id().as_u64();
        cx.update_global::<BrowsingSession, _>(|session, cx| {
            match session
                .windows
                .iter_mut()
                .find(|(window, _)| *window == key)
            {
                Some((_, recorded)) if *recorded == state => return,
                Some((_, recorded)) => *recorded = state,
                None => session.windows.push((key, state)),
            }
            session.save_soon(cx);
        });
    }

    /// Forgets this window as it closes, unless it is the last one, whose
    /// closing quits the browser and so is the session to reopen.
    pub(super) fn forget_session(key: u64, cx: &mut App) {
        cx.update_global::<BrowsingSession, _>(|session, cx| {
            if session.windows.len() > 1 {
                session.windows.retain(|(window, _)| *window != key);
                session.save_soon(cx);
            }
        });
    }

    /// The window's everyday tabs: not pinned ones or favourites', which
    /// their spaces reopen.
    fn session_state(&self, cx: &App) -> WindowSession {
        let mut state = WindowSession::default();
        for (index, tab) in self.tabs.iter().enumerate() {
            if tab.pin.is_some() || tab.favourite.is_some() {
                continue;
            }
            let summary = tab_summary(tab, cx);
            if summary.url.is_empty() {
                continue;
            }
            if self.active_tab == Some(index) {
                state.active = Some(state.tabs.len());
            }
            state.tabs.push(SessionTab {
                url: summary.url,
                title: summary.title,
                space: tab.space,
            });
        }
        state
    }

    /// Opens a saved window's tabs in their spaces, in the background, then
    /// shows the one that was in use.
    #[cfg(not(feature = "ui-only"))]
    fn restore_tabs(&mut self, saved: &WindowSession, window: &mut Window, cx: &mut Context<Self>) {
        let first = self.tabs.len();
        for tab in &saved.tabs {
            let index = self.tabs.len();
            match find_page(&tab.url) {
                Some(page) => {
                    let content = self.new_page(page, cx);
                    self.insert_tab_at_immediately(index, content, None, false, window, cx);
                }
                None => {
                    let Some(webview) = create_webview(cx, self.runtime.clone(), &tab.url) else {
                        continue;
                    };
                    webview.update(cx, |view, _| {
                        view.requested = Some(photon_core::Shortcut {
                            title: tab.title.clone(),
                            url: tab.url.clone(),
                        });
                    });
                    self.insert_webview_immediately(index, webview, None, false, window, cx);
                }
            }
            self.tabs[index].space = tab.space;
        }
        let active = saved.active.map_or(first, |position| first + position);
        if active < self.tabs.len() {
            self.activate_tab(active, true, window, cx);
        }
    }
}
