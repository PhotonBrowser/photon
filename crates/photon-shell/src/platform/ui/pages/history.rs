//! `photon://history`: the pages you visited, newest first and grouped by
//! day, with a search and a way to forget a page.

use gpui::{
    App, Context, ElementId, Entity, Focusable, ImageSource, ListAlignment, ListState, ObjectFit,
    Render, Role, Subscription, Window, div, img, list, prelude::*, px, rgb, rgba,
};
use gpui_elements::editable_text::{EditableTextState, StringStorage, text_input};
use photon_core::History;
use photon_core::{HistoryEntry, day_heading, local_day, time_of_day};
use photon_storage::{ImportSource, import_sources, read_history};
use std::sync::{Arc, Weak};
use std::time::{SystemTime, UNIX_EPOCH};

use super::super::button::{ButtonKind, ButtonSize, button};
use super::super::controls::text_field;
use super::super::history::BrowsingHistory;
use super::super::icons::{close_icon, globe_icon, history_icon};
use super::super::layout::{h_stack, v_stack};
use super::super::{metrics, theme::ThemeColors, theme::page_palette as palette};
use super::layout::{group, heading, secondary_text};
use super::{PageContext, PageDefinition, PageIcon};

pub(super) const PAGE: PageDefinition = PageDefinition {
    name: "history",
    title: "History",
    icon: PageIcon::Symbol(|color, size| history_icon(color, size).into_any_element()),
    shows_address: true,
    single_tab: true,
    build: |context, cx| cx.new(|cx| HistoryPage::new(context, cx)).into(),
};

pub(in super::super) struct HistoryPage {
    context: PageContext,
    search: Entity<EditableTextState>,
    list: ListState,
    /// Other browsers whose history can be imported.
    import_sources: Vec<ImportSource>,
    /// The browser being imported from.
    importing: Option<String>,
    /// The matching entries, newest first, kept until the history or the
    /// search changes rather than worked out again every frame.
    listing: Option<Listing>,
    pending: Option<(usize, String, u64)>,
    request_id: u64,
    _history: Subscription,
    _search: Subscription,
}

/// Indices into the history's pages that match a search, newest first.
struct Listing {
    history: Weak<History>,
    query: String,
    indices: Arc<Vec<usize>>,
}

impl HistoryPage {
    fn new(context: PageContext, cx: &mut Context<Self>) -> Self {
        // Finding other browsers' profiles reads the disk.
        cx.spawn(async move |this, cx| {
            let sources = cx
                .background_executor()
                .spawn(async { import_sources() })
                .await;
            this.update(cx, |this, cx| {
                this.import_sources = sources;
                cx.notify();
            })
            .ok();
        })
        .detach();
        let search = cx.new(|cx| EditableTextState::new(StringStorage::default(), cx));
        Self {
            context,
            _search: cx.observe(&search, |this, _, cx| {
                this.request_id = this.request_id.wrapping_add(1);
                this.pending = None;
                this.list.reset(0);
                cx.notify();
            }),
            search,
            list: ListState::new(0, ListAlignment::Top, px(metrics::LIST_ROW_ICON_SIZE)),
            import_sources: Vec::new(),
            importing: None,
            listing: None,
            pending: None,
            request_id: 0,
            // Follow visits, removals and icons arriving.
            _history: cx.observe_global::<BrowsingHistory>(|_, cx| cx.notify()),
        }
    }
}

impl HistoryPage {
    fn import(&mut self, source: ImportSource, cx: &mut Context<Self>) {
        if self.importing.is_some() {
            return;
        }
        self.importing = Some(source.name.clone());
        cx.notify();
        let read = cx.background_executor().spawn({
            let source = source.clone();
            async move { read_history(&source) }
        });
        cx.spawn(async move |this, cx| {
            let pages = read.await;
            this.update(cx, |this, cx| {
                this.importing = None;
                let message = match pages {
                    Ok(pages) => {
                        let added = BrowsingHistory::import_pages(pages, cx);
                        format!("Imported {added} pages from {}", source.name)
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::PermissionDenied => {
                        format!(
                            "Allow Full Disk Access in System Settings to import from {}",
                            source.name
                        )
                    }
                    Err(_) => format!("Couldn't import from {}", source.name),
                };
                crate::platform::ui::show_alert(cx, message);
                cx.notify();
            })
            .ok();
        })
        .detach();
    }

    fn import_section(
        &self,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> Option<impl IntoElement> {
        if self.import_sources.is_empty() {
            return None;
        }
        let busy = self.importing.is_some();
        let buttons = self
            .import_sources
            .iter()
            .enumerate()
            .map(|(index, source)| {
                let label = if self.importing.as_deref() == Some(source.name.as_str()) {
                    "Importing…".to_owned()
                } else {
                    source.name.clone()
                };
                let source = source.clone();
                button(
                    ElementId::NamedInteger("history-import".into(), index as u64),
                    label,
                    ButtonKind::Secondary,
                    !busy,
                    ButtonSize::Small,
                    palette,
                    Box::new(cx.listener(move |this, _, _, cx| this.import(source.clone(), cx))),
                )
            });
        Some(
            v_stack()
                .gap(px(metrics::MENU_ITEM_GAP * 3.0))
                .child(secondary_text(
                    "Import history from another browser",
                    palette,
                ))
                .child(
                    h_stack()
                        .flex_wrap()
                        .gap(px(metrics::MENU_ITEM_GAP * 3.0))
                        .children(buttons),
                ),
        )
    }
}

impl HistoryPage {
    /// Starts filtering away from the UI thread; stale results are discarded.
    fn load_listing(&mut self, history: Arc<History>, query: String, cx: &mut Context<Self>) {
        let key = Arc::as_ptr(&history) as usize;
        let current = self.listing.as_ref().is_some_and(|listing| {
            listing
                .history
                .upgrade()
                .is_some_and(|cached| Arc::ptr_eq(&cached, &history))
                && listing.query == query
        });
        if current
            || self
                .pending
                .as_ref()
                .is_some_and(|(pending_key, pending_query, _)| {
                    *pending_key == key && pending_query == &query
                })
        {
            return;
        }

        self.request_id = self.request_id.wrapping_add(1);
        let request_id = self.request_id;
        self.pending = Some((key, query.clone(), request_id));
        self.list.reset(0);
        let pages = history.clone();
        let worker_query = query.clone();
        let result = cx
            .background_executor()
            .spawn(async move { matching_pages(&pages, &worker_query) });
        cx.spawn(async move |this, cx| {
            let indices = result.await;
            this.update(cx, |this, cx| {
                if this.request_id != request_id {
                    return;
                }
                let current_query = this.search.read(cx).as_str().to_lowercase();
                let current_history = BrowsingHistory::pages(cx);
                if current_query != query || !Arc::ptr_eq(&current_history, &history) {
                    this.pending = None;
                    cx.notify();
                    return;
                }
                this.pending = None;
                this.listing = Some(Listing {
                    history: Arc::downgrade(&history),
                    query,
                    indices: Arc::new(indices),
                });
                this.list.reset(
                    this.listing
                        .as_ref()
                        .map_or(0, |listing| listing.indices.len()),
                );
                cx.notify();
            })
            .ok();
        })
        .detach();
    }
}

fn matching_pages(history: &History, query: &str) -> Vec<usize> {
    let words: Vec<String> = query.split_whitespace().map(str::to_owned).collect();
    let pages = history.pages();
    let mut indices: Vec<usize> = (0..pages.len())
        .filter(|&index| matches(&pages[index], &words))
        .collect();
    indices.sort_by_key(|&index| std::cmp::Reverse(pages[index].last_visit));
    indices
}

fn matches(entry: &HistoryEntry, words: &[String]) -> bool {
    let title = entry.title.to_lowercase();
    let url = entry.url.to_lowercase();
    words
        .iter()
        .all(|word| title.contains(word) || url.contains(word))
}

/// How far local time is ahead of UTC.
#[cfg(unix)]
fn utc_offset() -> i64 {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as libc::time_t);
    let mut local = unsafe { std::mem::zeroed::<libc::tm>() };
    if unsafe { libc::localtime_r(&now, &mut local) }.is_null() {
        return 0;
    }
    local.tm_gmtoff as i64
}

#[cfg(not(unix))]
fn utc_offset() -> i64 {
    0
}

/// The address without its scheme, `www.` or trailing slash, as a row's detail.
fn display_address(url: &str) -> String {
    let rest = url.split_once("://").map_or(url, |(_, rest)| rest);
    let rest = rest.strip_prefix("www.").unwrap_or(rest);
    rest.strip_suffix('/').unwrap_or(rest).to_owned()
}

fn page_icon(url: &str, palette: ThemeColors, cx: &mut App) -> gpui::AnyElement {
    let size = metrics::LIST_ROW_ICON_SIZE;
    match BrowsingHistory::favicon(url, cx) {
        Some(favicon) => img(ImageSource::Render(favicon.image))
            .size(px(size))
            .object_fit(ObjectFit::Contain)
            .into_any_element(),
        None => globe_icon(palette.text_secondary, size).into_any_element(),
    }
}

impl HistoryPage {
    fn row(
        context: &PageContext,
        index: usize,
        entry: &HistoryEntry,
        utc_offset: i64,
        palette: ThemeColors,
        cx: &mut App,
    ) -> impl IntoElement {
        let title = if entry.title.trim().is_empty() {
            entry.url.clone()
        } else {
            entry.title.clone()
        };
        let open_url = entry.url.clone();
        let context = context.clone();
        let remove_url = entry.url.clone();
        let group_name: gpui::SharedString = format!("history-row-{index}").into();
        h_stack()
            .id(ElementId::NamedInteger("history-row".into(), index as u64))
            .group(group_name.clone())
            .role(Role::Link)
            .aria_label(title.clone())
            .w_full()
            .items_center()
            .gap(px(metrics::LIST_ROW_GAP))
            .px(px(metrics::LIST_ROW_PADDING))
            .py(px(metrics::LIST_ROW_PADDING * 0.7))
            .rounded(px(metrics::menu_item_radius()))
            .cursor_pointer()
            .hover_style(|style| style.bg(rgba(palette.hover_surface)))
            .on_click(move |_, _, cx| context.open(open_url.clone(), cx))
            .child(page_icon(&entry.url, palette, cx))
            .child(
                v_stack()
                    .flex_1()
                    .min_w_0()
                    .gap(px(metrics::HISTORY_ROW_LINE_GAP))
                    .child(
                        div()
                            .truncate()
                            .text_size(px(metrics::MENU_FONT_SIZE))
                            .text_color(rgb(palette.text_primary))
                            .child(title),
                    )
                    .child(
                        div()
                            .truncate()
                            .text_size(px(metrics::HISTORY_ROW_DETAIL_FONT_SIZE))
                            .text_color(rgb(palette.text_secondary))
                            .child(display_address(&entry.url)),
                    ),
            )
            .child(
                div()
                    .flex_shrink_0()
                    .text_size(px(metrics::HISTORY_ROW_DETAIL_FONT_SIZE))
                    .text_color(rgb(palette.text_secondary))
                    .child(time_of_day(entry.last_visit, utc_offset)),
            )
            .child(
                h_stack()
                    .id(ElementId::NamedInteger(
                        "history-remove".into(),
                        index as u64,
                    ))
                    .role(Role::Button)
                    .aria_label("Remove from history")
                    .flex_shrink_0()
                    .items_center()
                    .justify_center()
                    .size(px(metrics::HISTORY_ROW_REMOVE_SIZE))
                    .rounded_full()
                    .tab_stop(false)
                    .opacity(0.0)
                    .group_hover_style(group_name, |style| style.opacity(1.0))
                    .hover_style(|style| style.bg(rgba(palette.hover_surface)))
                    .on_click(move |_, _, cx| {
                        cx.stop_propagation();
                        BrowsingHistory::remove_page(&remove_url, cx);
                    })
                    .child(close_icon(
                        palette.text_secondary,
                        metrics::HISTORY_ROW_REMOVE_ICON_SIZE,
                    )),
            )
    }
}

fn history_skeleton(palette: ThemeColors) -> impl IntoElement {
    let placeholder = rgba(palette.menu_border);
    v_stack()
        .w_full()
        .gap(px(metrics::MENU_ITEM_GAP * 3.0))
        .child(secondary_text("Loading history…", palette))
        .children((0..6).map(|_| {
            h_stack()
                .w_full()
                .items_center()
                .gap(px(metrics::LIST_ROW_GAP))
                .p(px(metrics::LIST_ROW_PADDING))
                .rounded(px(metrics::surface_radius()))
                .bg(rgba(palette.menu_surface))
                .child(
                    div()
                        .size(px(metrics::LIST_ROW_ICON_SIZE))
                        .rounded_full()
                        .bg(placeholder),
                )
                .child(
                    v_stack()
                        .flex_1()
                        .gap(px(metrics::HISTORY_ROW_LINE_GAP))
                        .child(
                            div()
                                .w_full()
                                .max_w(px(metrics::INTERNAL_PAGE_MAX_WIDTH * 0.42))
                                .h(px(metrics::MENU_FONT_SIZE))
                                .rounded(px(metrics::control_radius()))
                                .bg(placeholder),
                        )
                        .child(
                            div()
                                .w_full()
                                .max_w(px(metrics::INTERNAL_PAGE_MAX_WIDTH * 0.26))
                                .h(px(metrics::HISTORY_ROW_DETAIL_FONT_SIZE))
                                .rounded(px(metrics::control_radius()))
                                .bg(placeholder),
                        ),
                )
        }))
}

impl Render for HistoryPage {
    fn render(&mut self, window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let palette = palette(window, cx);
        let history = BrowsingHistory::pages(cx);
        let query = self.search.read(cx).as_str().to_lowercase();
        let searching = !query.trim().is_empty();
        self.load_listing(history.clone(), query.clone(), cx);
        let indices = self.listing.as_ref().and_then(|listing| {
            (listing
                .history
                .upgrade()
                .is_some_and(|cached| Arc::ptr_eq(&cached, &history))
                && listing.query == query)
                .then(|| listing.indices.clone())
        });

        let offset = utc_offset();
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs());
        let today = local_day(now, offset);

        let search = text_field(
            text_input("history-search")
                .state(self.search.downgrade())
                .track_focus(&self.search.focus_handle(cx))
                .aria_label("Search history")
                .placeholder("Search history")
                .placeholder_color(rgb(palette.text_secondary)),
            palette,
        );

        let list_content: gpui::AnyElement = if let Some(indices) = indices {
            if indices.is_empty() {
                let message = if searching {
                    "No pages match your search."
                } else {
                    "Pages you visit will appear here."
                };
                group(palette)
                    .p(px(metrics::LIST_ROW_PADDING))
                    .child(secondary_text(message, palette))
                    .into_any_element()
            } else {
                let list_state = self.list.clone();
                let pages = history.clone();
                let context = self.context.clone();
                let item_indices = indices.clone();
                list(list_state, move |index, _window, app| {
                    let entry_index = item_indices[index];
                    let entry = &pages.pages()[entry_index];
                    let day = local_day(entry.last_visit, offset);
                    let is_first_of_day = index == 0
                        || local_day(pages.pages()[item_indices[index - 1]].last_visit, offset)
                            != day;
                    let row = HistoryPage::row(&context, entry_index, entry, offset, palette, app)
                        .into_any_element();
                    let mut item = v_stack().gap(px(metrics::MENU_ITEM_GAP * 2.0));
                    if is_first_of_day {
                        item = item.child(
                            div()
                                .px(px(metrics::LIST_ROW_PADDING))
                                .text_size(px(metrics::MENU_FONT_SIZE))
                                .font_weight(gpui::FontWeight::SEMIBOLD)
                                .text_color(rgb(palette.text_secondary))
                                .child(day_heading(day, today)),
                        );
                    }
                    item.child(group(palette).child(row)).into_any_element()
                })
                .w_full()
                .h_full()
                .into_any_element()
            }
        } else {
            history_skeleton(palette).into_any_element()
        };

        let context = self.context.clone();
        let header = h_stack()
            .w_full()
            .items_end()
            .justify_between()
            .gap(px(metrics::LIST_ROW_GAP))
            .child(heading(
                "history-heading",
                "History",
                "Pages you visited, newest first.",
                palette,
            ))
            .child(button(
                "history-clear",
                "Clear Browsing Data…",
                ButtonKind::Secondary,
                true,
                ButtonSize::Small,
                palette,
                Box::new(move |_, _, cx| context.open_page(super::SETTINGS, cx)),
            ));
        v_stack()
            .id("history-page")
            .size_full()
            .items_center()
            .overflow_hidden()
            .p(px(metrics::INTERNAL_PAGE_INSET))
            .child(
                v_stack()
                    .size_full()
                    .max_w(px(metrics::INTERNAL_PAGE_MAX_WIDTH))
                    .gap(px(metrics::INTERNAL_PAGE_SECTION_GAP))
                    .child(header)
                    .children(self.import_section(palette, cx))
                    .child(search)
                    .child(div().flex_1().min_h(px(0.)).w_full().child(list_content)),
            )
    }
}
