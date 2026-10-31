//! The sidebar's downloads button: a ring around its icon follows the
//! downloads in progress, and a dot says a download arrived since the
//! downloads popover was last opened.

use gpui::{PathBuilder, Pixels, Point, Window, canvas, div, point, prelude::*, px, rgb, rgba};
use std::{
    f32::consts::TAU,
    sync::OnceLock,
    time::{Duration, Instant},
};

use super::button::toolbar_button;
use super::downloads::DownloadProgress;
use super::icons::download_icon;
use super::{ClickHandler, metrics, theme::ThemeColors};

pub(super) struct DownloadsButton {
    pub(super) progress: Option<DownloadProgress>,
    pub(super) unseen: bool,
    /// Whether the ring may spin while a download's size is unknown.
    pub(super) animate: bool,
}

pub(super) fn downloads_button(
    id: &'static str,
    tab_index: isize,
    state: DownloadsButton,
    palette: ThemeColors,
    on_click: ClickHandler,
) -> impl IntoElement {
    let icon_color = if state.progress.is_some() || state.unseen {
        palette.text_primary
    } else {
        palette.text_secondary
    };
    let content = div()
        .relative()
        .size_full()
        .flex()
        .items_center()
        .justify_center()
        .child(download_icon(icon_color, metrics::TOOLBAR_ICON_SIZE))
        .children(
            state
                .progress
                .map(|progress| progress_ring(progress, state.animate, palette)),
        )
        // The ring already says something is downloading; the dot waits for it to end.
        .when(state.unseen && state.progress.is_none(), |content| {
            content.child(unseen_dot(palette))
        });
    toolbar_button(id, "Downloads", tab_index, true, content, palette, on_click)
}

fn unseen_dot(palette: ThemeColors) -> impl IntoElement {
    div()
        .absolute()
        .top(px(metrics::DOWNLOAD_BADGE_INSET))
        .right(px(metrics::DOWNLOAD_BADGE_INSET))
        .size(px(metrics::DOWNLOAD_BADGE_SIZE))
        .rounded_full()
        .bg(rgb(palette.chosen))
}

fn progress_ring(
    progress: DownloadProgress,
    animate: bool,
    palette: ThemeColors,
) -> impl IntoElement {
    let track = palette.menu_border;
    let ink = palette.chosen;
    canvas(
        |_, _, _| {},
        move |bounds, _, window: &mut Window, _| {
            let radius = f32::from(bounds.size.width.min(bounds.size.height)) / 2.0
                - metrics::DOWNLOAD_RING_INSET;
            if radius <= 0.0 {
                return;
            }
            let center = bounds.center();
            paint_arc(window, center, radius, 0.0, 1.0, rgba(track));
            let (start, sweep) = match progress.fraction {
                Some(fraction) => (0.0, fraction),
                None => {
                    if animate {
                        window.request_animation_frame();
                    }
                    (
                        spin_turn(animate),
                        metrics::DOWNLOAD_RING_INDETERMINATE_SWEEP,
                    )
                }
            };
            paint_arc(window, center, radius, start, sweep, rgb(ink));
        },
    )
    .absolute()
    .inset_0()
}

/// How far round the spinning arc has turned, as a share of a turn.
fn spin_turn(animate: bool) -> f32 {
    if !animate {
        return 0.0;
    }
    static EPOCH: OnceLock<Instant> = OnceLock::new();
    let period = Duration::from_millis(metrics::DOWNLOAD_RING_SPIN_MS);
    let elapsed = EPOCH.get_or_init(Instant::now).elapsed();
    (elapsed.as_secs_f32() % period.as_secs_f32()) / period.as_secs_f32()
}

/// Strokes the arc that starts `start` of a turn clockwise from the top and
/// covers `sweep` of a turn.
fn paint_arc(
    window: &mut Window,
    center: Point<Pixels>,
    radius: f32,
    start: f32,
    sweep: f32,
    color: impl Into<gpui::Background>,
) {
    // An arc that ends where it starts draws nothing, so a full ring stops
    // just short.
    let sweep = sweep.clamp(0.0, 0.999);
    if sweep <= 0.0 {
        return;
    }
    let at = |turn: f32| {
        let angle = turn * TAU;
        center + point(px(angle.sin() * radius), px(-angle.cos() * radius))
    };
    let mut path = PathBuilder::stroke(px(metrics::DOWNLOAD_RING_STROKE));
    path.move_to(at(start));
    path.arc_to(
        point(px(radius), px(radius)),
        px(0.0),
        sweep > 0.5,
        true,
        at(start + sweep),
    );
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
}
