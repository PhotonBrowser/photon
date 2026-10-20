//! The title and address a tab contributes to the window and saved session.

use gpui::App;

use super::content::{BrowserTab, TabContent};

pub(super) struct TabSummary {
    pub(super) title: String,
    pub(super) url: String,
}

/// A tab's title, or its address when the page has none, and its address.
pub(super) fn tab_summary(tab: &BrowserTab, cx: &App) -> TabSummary {
    match &tab.content {
        TabContent::Page(page) => TabSummary {
            title: page.definition.title.to_owned(),
            url: tab.content.page_address().unwrap_or_default(),
        },
        TabContent::Sleeping(sleeping) => {
            let state = &sleeping.state;
            TabSummary {
                title: if state.title.trim().is_empty() {
                    state.url.clone()
                } else {
                    state.title.clone()
                },
                url: state.url.clone(),
            }
        }
        TabContent::Web(webview) => {
            let view = webview.read(cx);
            // Until the page loads, it is what the tab was pinned or opened as.
            if !view.has_page()
                && let Some(saved) = tab.pin.as_ref().or(view.requested.as_ref())
            {
                return TabSummary {
                    title: if saved.title.trim().is_empty() {
                        saved.url.clone()
                    } else {
                        saved.title.clone()
                    },
                    url: saved.url.clone(),
                };
            }
            let state = &view.state;
            TabSummary {
                title: if state.title.trim().is_empty() {
                    state.url.clone()
                } else {
                    state.title.clone()
                },
                url: state.url.clone(),
            }
        }
    }
}
