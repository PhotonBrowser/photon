//! Addresses the operating system hands the browser, as when a link is opened
//! from another app: each opens in a new tab of the frontmost browser window,
//! or of a new window when there is none.

use futures::StreamExt;
use futures::channel::mpsc::{UnboundedReceiver, UnboundedSender, unbounded};
use gpui::{App, Application};
use std::rc::Rc;

use super::super::super::app_update::update_app;
use super::super::super::engine::EngineRuntime;
use super::super::super::trace;
use super::{BrowserWindow, open_browser_window};

/// Asks the platform for the addresses it opens, before the app runs, as it
/// may hand some over at launch.
pub(super) fn receive_opened_urls(application: &Application) -> UnboundedReceiver<Vec<String>> {
    let (sender, receiver): (UnboundedSender<Vec<String>>, _) = unbounded();
    application.on_open_urls(move |urls| {
        sender.unbounded_send(urls).ok();
    });
    receiver
}

/// Opens the addresses as they arrive.
pub(super) fn open_received_urls(
    mut urls: UnboundedReceiver<Vec<String>>,
    runtime: Rc<EngineRuntime>,
    cx: &mut App,
) {
    cx.spawn(async move |cx| {
        while let Some(urls) = urls.next().await {
            update_app(cx, |cx| open_urls(&urls, &runtime, cx)).await;
        }
    })
    .detach();
}

fn open_urls(urls: &[String], runtime: &Rc<EngineRuntime>, cx: &mut App) {
    let Some((first, rest)) = urls.split_first() else {
        return;
    };
    cx.activate(true);
    let frontmost = cx
        .active_window()
        .and_then(|window| window.downcast::<BrowserWindow>())
        .or_else(|| {
            cx.windows()
                .into_iter()
                .find_map(|window| window.downcast::<BrowserWindow>())
        });
    let (window, rest) = match frontmost {
        Some(window) => (window, urls),
        None => match open_browser_window(runtime.clone(), Some(first.clone()), false, false, cx) {
            Ok(window) => (window, rest),
            Err(error) => {
                trace(format_args!("open url: {error:#}"));
                return;
            }
        },
    };
    window
        .update(cx, |browser, window, cx| {
            for url in rest {
                browser.open_in_new_tab(url, window, cx);
            }
            window.activate();
        })
        .ok();
}
