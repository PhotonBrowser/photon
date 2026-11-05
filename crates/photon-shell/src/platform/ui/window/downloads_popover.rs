//! The downloads popover, opened from the sidebar's downloads button: the
//! latest downloads, their progress, and the way to the full list.

use gpui::{
    AnyElement, App, Context, ElementId, KeyDownEvent, MouseButton, Role, Window, div, prelude::*,
    px, relative, rgb, rgba,
};
use std::rc::Rc;

use super::super::super::engine::{Download, DownloadStatus, EngineRuntime};
use super::super::button::icon_button;
use super::super::controls::OwnsClicks;
use super::super::downloads::{Downloads, FileKind, file_name, progress_fraction, status_text};
use super::super::icons::{close_icon, file_icon, folder_open_icon, open_in_tab_icon};
use super::super::layout::{h_stack, v_stack};
use super::super::menu::popover_surface;
use super::super::motion::{Edge, SurfaceFade, Transition};
use super::super::pages::DOWNLOADS;
use super::super::settings::Settings;
use super::super::{metrics, theme::ThemeColors, theme::palette};
use super::BrowserWindow;
use photon_core::Feature;

/// What the popover's controls ask the window to do.
#[derive(Clone)]
struct PopoverActions {
    close: Rc<dyn Fn(&mut App)>,
    show_all: Rc<dyn Fn(&mut App)>,
    /// Whether the window is private, which lists only private downloads.
    private: bool,
}

impl BrowserWindow {
    /// Opens the downloads popover above the sidebar's downloads button, or
    /// closes it when it is open.
    pub(super) fn toggle_downloads_popover(&mut self, window: &Window, cx: &mut Context<Self>) {
        if !Settings::get(cx).features.is_enabled(Feature::Downloads) {
            return;
        }
        if self.downloads_popover.is_some() {
            self.dismiss_downloads_popover(cx);
            return;
        }
        Downloads::mark_seen(self.private, cx);
        // Below its button where the sidebar shows it, else where it was asked for.
        let anchor = self.downloads_button_bounds.unwrap_or_else(|| {
            gpui::Bounds::new(window.mouse_position(), gpui::size(px(0.0), px(0.0)))
        });
        self.downloads_popover = Some(anchor);
        cx.notify();
    }

    pub(super) fn dismiss_downloads_popover(&mut self, cx: &mut Context<Self>) {
        if self.downloads_popover.take().is_some() {
            cx.notify();
        }
    }

    fn downloads_popover_actions(&self, window: &Window, cx: &Context<Self>) -> PopoverActions {
        let browser = cx.entity().downgrade();
        let browser_window = window.window_handle();
        let close = {
            let browser = browser.clone();
            Rc::new(move |cx: &mut App| {
                let _ = browser.update(cx, |this, cx| this.dismiss_downloads_popover(cx));
            })
        };
        let show_all = Rc::new(move |cx: &mut App| {
            let browser = browser.clone();
            let _ = browser_window.update(cx, |_, window, cx| {
                let _ = browser.update(cx, |this, cx| {
                    this.dismiss_downloads_popover(cx);
                    this.open_page_tab(DOWNLOADS, window, cx);
                });
            });
        });
        PopoverActions {
            close,
            show_all,
            private: self.private,
        }
    }

    /// The popover, drawn in the window above its button. Its height follows
    /// the downloads listed, so it grows as downloads arrive.
    pub(super) fn downloads_popover_overlay(
        &mut self,
        window: &Window,
        cx: &mut Context<Self>,
    ) -> Option<AnyElement> {
        if !Settings::get(cx).features.is_enabled(Feature::Downloads) {
            return None;
        }
        let anchor = self.downloads_popover?;
        let palette = palette(window, cx);
        let actions = self.downloads_popover_actions(window, cx);
        let viewport = window.viewport_size();
        let width = metrics::DOWNLOADS_POPOVER_WIDTH;
        let height = popover_height(Downloads::list(self.private, cx).count());
        let inset = metrics::PAGE_INSET;
        let left = (f32::from(anchor.center().x) - width / 2.0).clamp(
            inset,
            (f32::from(viewport.width) - width - inset).max(inset),
        );
        // Just above the button, leaving it uncovered.
        let top = (f32::from(anchor.origin.y) - height - metrics::PAGE_CHROME_GAP).clamp(
            inset,
            (f32::from(viewport.height) - height - inset).max(inset),
        );
        // The surface positions its own content, so a wrapper places it.
        let panel = div().absolute().left(px(left)).top(px(top)).child(
            popover_surface(
                "Recent downloads",
                Role::Dialog,
                width,
                Edge::Bottom,
                popover_content(&self.runtime, &actions, palette, cx),
                Transition::Enter,
                palette,
            )
            .h(px(height)),
        );
        let close = actions.close.clone();
        let close_on_key = actions.close.clone();
        let _ = Settings::motion(cx);
        Some(
            div()
                .id("downloads-popover-overlay")
                .absolute()
                .inset_0()
                .occlude()
                .on_mouse_down(MouseButton::Left, move |_, _, cx| close(cx))
                .capture_key_down(move |event: &KeyDownEvent, _, cx| {
                    if event.keystroke.key.as_str() == "escape"
                        && !event.keystroke.modifiers.modified()
                    {
                        cx.stop_propagation();
                        close_on_key(cx);
                    }
                })
                .child(panel)
                .into_any_element(),
        )
    }
}

/// The popover's height for `count` downloads, showing at most a few.
fn popover_height(count: usize) -> f32 {
    let rows = count.clamp(1, metrics::DOWNLOADS_POPOVER_MAX_ROWS) as f32;
    metrics::DOWNLOADS_POPOVER_HEADER_HEIGHT
        + rows * metrics::DOWNLOADS_POPOVER_ROW_HEIGHT
        + metrics::DOWNLOADS_POPOVER_FOOTER_HEIGHT
        + metrics::MENU_PADDING * 2.0
}

fn popover_content(
    runtime: &Rc<EngineRuntime>,
    actions: &PopoverActions,
    palette: ThemeColors,
    cx: &App,
) -> AnyElement {
    let downloads: Vec<_> = Downloads::list(actions.private, cx)
        .take(metrics::DOWNLOADS_POPOVER_MAX_ROWS)
        .collect();
    let close = actions.close.clone();
    let header = h_stack()
        .h(px(metrics::DOWNLOADS_POPOVER_HEADER_HEIGHT))
        .flex_shrink_0()
        .items_center()
        .justify_between()
        .px(px(metrics::LIST_ROW_PADDING))
        .child(
            div()
                .text_size(px(metrics::DOWNLOADS_POPOVER_TITLE_FONT_SIZE))
                .font_weight(gpui::FontWeight::SEMIBOLD)
                .text_color(rgb(palette.text_primary))
                .child("Recent downloads"),
        )
        .child(icon_button(
            "downloads-popover-close",
            "Close",
            true,
            metrics::CHIP_CLOSE_SIZE,
            close_icon(palette.text_secondary, metrics::CHIP_CLOSE_ICON_SIZE),
            palette,
            Box::new(move |_, _, cx| close(cx)),
        ));
    let rows: Vec<AnyElement> = if downloads.is_empty() {
        vec![
            div()
                .h(px(metrics::DOWNLOADS_POPOVER_ROW_HEIGHT))
                .flex()
                .items_center()
                .px(px(metrics::LIST_ROW_PADDING))
                .text_size(px(metrics::MENU_FONT_SIZE))
                .text_color(rgb(palette.text_secondary))
                .child("Files you download appear here.")
                .into_any_element(),
        ]
    } else {
        downloads
            .iter()
            .map(|download| download_row(runtime, download, palette).into_any_element())
            .collect()
    };
    let show_all = actions.show_all.clone();
    let footer = h_stack()
        .id("downloads-popover-show-all")
        .role(Role::Link)
        .aria_label("Full download history")
        .h(px(metrics::DOWNLOADS_POPOVER_FOOTER_HEIGHT))
        .flex_shrink_0()
        .items_center()
        .justify_between()
        .px(px(metrics::LIST_ROW_PADDING))
        .rounded(px(metrics::control_radius()))
        .surface_fade()
        .hover_style(|style| style.bg(rgba(palette.hover_surface)))
        .owns_clicks()
        .on_click(move |_, _, cx| show_all(cx))
        .text_size(px(metrics::MENU_FONT_SIZE))
        .text_color(rgb(palette.text_primary))
        .child("Full download history")
        .child(open_in_tab_icon(
            palette.text_secondary,
            metrics::CHIP_ICON_SIZE,
        ));
    v_stack()
        .id("downloads-popover")
        .size_full()
        .owns_clicks()
        .child(header)
        .child(v_stack().flex_1().min_h(px(0.)).children(rows))
        .child(footer)
        .into_any_element()
}

fn download_row(
    runtime: &Rc<EngineRuntime>,
    download: &Download,
    palette: ThemeColors,
) -> impl IntoElement {
    let id = download.id;
    let name = file_name(download);
    let completed = download.status == DownloadStatus::Completed;
    let active = download.status.is_active();
    let path = download.path.clone();
    let trailing: Option<AnyElement> = if completed {
        let path = download.path.clone();
        Some(
            icon_button(
                "download-reveal",
                "Show in folder",
                true,
                metrics::CHIP_CLOSE_SIZE,
                folder_open_icon(palette.text_secondary, metrics::CHIP_CLOSE_ICON_SIZE),
                palette,
                Box::new(move |_, _, cx| {
                    cx.stop_propagation();
                    cx.reveal_path(&path);
                }),
            )
            .into_any_element(),
        )
    } else if active {
        let runtime = runtime.clone();
        Some(
            icon_button(
                "download-cancel",
                "Cancel download",
                true,
                metrics::CHIP_CLOSE_SIZE,
                close_icon(palette.text_secondary, metrics::CHIP_CLOSE_ICON_SIZE),
                palette,
                Box::new(move |_, _, cx| {
                    cx.stop_propagation();
                    runtime.cancel_download(id);
                }),
            )
            .into_any_element(),
        )
    } else {
        None
    };
    let progress = active.then(|| {
        div()
            .w_full()
            .h(px(metrics::DOWNLOAD_PROGRESS_HEIGHT))
            .rounded_full()
            .bg(rgba(palette.menu_border))
            .child(
                div()
                    .h_full()
                    .w(relative(progress_fraction(download).unwrap_or(0.0)))
                    .rounded_full()
                    .bg(rgb(palette.chosen)),
            )
    });
    let mut row = h_stack()
        .id(ElementId::Name(format!("download-row-{id}").into()))
        .role(if completed {
            Role::Button
        } else {
            Role::ListItem
        })
        .aria_label(format!("{name}, {}", status_text(download)))
        .h(px(metrics::DOWNLOADS_POPOVER_ROW_HEIGHT))
        .flex_shrink_0()
        .items_center()
        .gap(px(metrics::LIST_ROW_GAP))
        .px(px(metrics::LIST_ROW_PADDING))
        .rounded(px(metrics::control_radius()))
        .child(
            file_icon(
                FileKind::of(download),
                palette.text_secondary,
                metrics::DOWNLOADS_POPOVER_FILE_ICON_SIZE,
            )
            .flex_shrink_0(),
        )
        .child(
            v_stack()
                .flex_1()
                .min_w_0()
                .gap(px(metrics::MENU_ITEM_GAP))
                .child(
                    div()
                        .truncate()
                        .text_size(px(metrics::MENU_FONT_SIZE))
                        .text_color(rgb(palette.text_primary))
                        .child(name),
                )
                .children(progress)
                .child(
                    div()
                        .truncate()
                        .text_size(px(metrics::HISTORY_ROW_DETAIL_FONT_SIZE))
                        .text_color(rgb(palette.text_secondary))
                        .child(status_text(download)),
                ),
        )
        .children(trailing);
    if completed {
        row = row
            .surface_fade()
            .hover_style(|style| style.bg(rgba(palette.hover_surface)))
            .owns_clicks()
            .on_click(move |_, _, cx| cx.open_with_system(&path));
    }
    row
}
