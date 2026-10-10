//! The privacy section: clearing browsing data from a chosen time range,
//! with how much disk each kind takes.

use gpui::{Context, ElementId, SharedString, Task, prelude::*, px};
use photon_core::ClearBrowsingData;
use photon_storage::DataUsage;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use super::super::super::button::{ButtonSize, button};
use super::super::super::controls::{Choice, choices};
use super::super::super::history::BrowsingHistory;
use super::super::super::layout::{h_stack, v_stack};
use super::super::super::{metrics, theme::ThemeColors};
use super::super::controls::checkbox_row;
use super::super::layout::{group, label, secondary_text};
use super::SettingsPage;

/// What clearing will delete, and how it is going.
pub(super) struct ClearForm {
    range: ClearRange,
    request: ClearBrowsingData,
    state: ClearState,
    /// Disk used by the profile's data, once measured.
    usage: Option<DataUsage>,
    _measuring: Option<Task<()>>,
}

#[derive(Clone, Copy, PartialEq)]
enum ClearState {
    Ready,
    Clearing,
    Cleared,
}

/// How far back clearing reaches.
#[derive(Clone, Copy, PartialEq)]
enum ClearRange {
    LastHour,
    LastDay,
    LastWeek,
    AllTime,
}

impl ClearRange {
    const ALL: [Self; 4] = [Self::LastHour, Self::LastDay, Self::LastWeek, Self::AllTime];

    fn label(self) -> &'static str {
        match self {
            Self::LastHour => "Last hour",
            Self::LastDay => "Last 24 hours",
            Self::LastWeek => "Last 7 days",
            Self::AllTime => "All time",
        }
    }

    /// The earliest time cleared, in Unix seconds.
    fn since(self) -> u64 {
        let reach = match self {
            Self::LastHour => Duration::from_secs(60 * 60),
            Self::LastDay => Duration::from_secs(24 * 60 * 60),
            Self::LastWeek => Duration::from_secs(7 * 24 * 60 * 60),
            Self::AllTime => return 0,
        };
        SystemTime::now()
            .checked_sub(reach)
            .and_then(|since| since.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |since| since.as_secs())
    }
}

/// A kind of data the form can clear.
struct DataKind {
    id: &'static str,
    label: &'static str,
    selected: fn(&mut ClearBrowsingData) -> &mut bool,
    size: fn(&DataUsage) -> u64,
}

const KINDS: [DataKind; 4] = [
    DataKind {
        id: "history",
        label: "Browsing history",
        selected: |request| &mut request.history,
        size: |usage| usage.history + usage.favicons,
    },
    DataKind {
        id: "searches",
        label: "Search history",
        selected: |request| &mut request.searches,
        // Saved with the browsing history.
        size: |_| 0,
    },
    DataKind {
        id: "cache",
        label: "Cached files",
        selected: |request| &mut request.cache,
        size: |usage| usage.cache,
    },
    DataKind {
        id: "site-data",
        label: "Cookies and site data",
        selected: |request| &mut request.site_data,
        size: |usage| usage.site_data,
    },
];

impl ClearForm {
    pub(super) fn new(cx: &mut Context<SettingsPage>) -> Self {
        let mut form = Self {
            range: ClearRange::LastHour,
            request: ClearBrowsingData {
                history: true,
                searches: true,
                cache: true,
                ..ClearBrowsingData::default()
            },
            state: ClearState::Ready,
            usage: None,
            _measuring: None,
        };
        form.measure(cx);
        form
    }

    fn measure(&mut self, cx: &mut Context<SettingsPage>) {
        let measuring = BrowsingHistory::measure_data_usage(cx);
        self._measuring = Some(cx.spawn(async move |page, cx| {
            let usage = measuring.await;
            page.update(cx, |page, cx| {
                page.clearing.usage = usage;
                cx.notify();
            })
            .ok();
        }));
    }

    fn is_selected(&self, kind: &DataKind) -> bool {
        *(kind.selected)(&mut self.request.clone())
    }
}

impl SettingsPage {
    fn edit_clearing(&mut self, cx: &mut Context<Self>, edit: impl FnOnce(&mut ClearForm)) {
        edit(&mut self.clearing);
        self.clearing.state = ClearState::Ready;
        cx.notify();
    }

    fn clear(&mut self, cx: &mut Context<Self>) {
        let request = ClearBrowsingData {
            since: self.clearing.range.since(),
            ..self.clearing.request
        };
        self.clearing.state = ClearState::Clearing;
        let page = cx.entity().downgrade();
        BrowsingHistory::clear(
            request,
            &self.context.runtime,
            move |cx| {
                page.update(cx, |page, cx| {
                    page.clearing.state = ClearState::Cleared;
                    page.clearing.measure(cx);
                    cx.notify();
                })
                .ok();
            },
            cx,
        );
        cx.notify();
    }

    pub(super) fn privacy_settings(
        &self,
        palette: ThemeColors,
        cx: &mut Context<Self>,
    ) -> impl IntoElement {
        let form = &self.clearing;
        let ranges = ClearRange::ALL
            .into_iter()
            .map(|range| Choice {
                label: range.label().into(),
                chosen: form.range == range,
                on_choose: Box::new(cx.listener(move |this, _, _, cx| {
                    this.edit_clearing(cx, |form| form.range = range);
                })),
            })
            .collect();
        let kinds = KINDS.iter().enumerate().map(|(index, kind)| {
            let size = form.usage.map_or(0, |usage| (kind.size)(&usage));
            let label = if size > 0 {
                SharedString::from(format!("{} · {}", kind.label, format_bytes(size)))
            } else {
                SharedString::from(kind.label)
            };
            checkbox_row(
                (ElementId::from("settings-clear"), kind.id),
                label,
                form.is_selected(kind),
                palette,
                Box::new(cx.listener(move |this, _, _, cx| {
                    this.edit_clearing(cx, |form| {
                        let selected = (KINDS[index].selected)(&mut form.request);
                        *selected = !*selected;
                    });
                })),
            )
        });
        let anything = KINDS.iter().any(|kind| form.is_selected(kind));
        let (button_label, clickable) = match form.state {
            ClearState::Ready => ("Clear data", anything),
            ClearState::Clearing => ("Clearing…", false),
            ClearState::Cleared => ("Cleared", false),
        };
        let total = form.usage.map_or_else(
            || "Measuring…".to_owned(),
            |usage| format!("{} used in total", format_bytes(usage.total())),
        );
        v_stack()
            .gap(px(metrics::INTERNAL_PAGE_GROUP_GAP))
            .child(
                v_stack()
                    .gap(px(metrics::MENU_ITEM_GAP))
                    .child(label("Time range", palette))
                    .child(choices(
                        "settings-clear-range",
                        "Time range",
                        ranges,
                        palette,
                    )),
            )
            .child(group(palette).children(kinds))
            .child(
                h_stack()
                    .items_center()
                    .justify_between()
                    .child(secondary_text(total, palette))
                    .child(button(
                        "settings-clear-data",
                        button_label,
                        true,
                        ButtonSize::Regular,
                        palette,
                        Box::new(cx.listener(move |this, _, _, cx| {
                            if clickable {
                                this.clear(cx);
                            }
                        })),
                    )),
            )
    }
}

/// A size such as "12.4 MB".
fn format_bytes(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["bytes", "KB", "MB", "GB"];
    let mut size = bytes as f64;
    let mut unit = 0;
    while size >= 1000.0 && unit < UNITS.len() - 1 {
        size /= 1000.0;
        unit += 1;
    }
    if unit == 0 {
        format!("{bytes} {}", UNITS[0])
    } else {
        format!("{size:.1} {}", UNITS[unit])
    }
}
