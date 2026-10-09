//! GPUI presentation for performance diagnostics.

use crate::PerformanceDiagnostics;
use gpui::{Div, div, prelude::*, px, rgb, rgba};

const OVERLAY_INSET: f32 = 12.0;
const OVERLAY_GAP: f32 = 2.0;
const OVERLAY_MIN_WIDTH: f32 = 236.0;
const OVERLAY_HORIZONTAL_PADDING: f32 = 8.0;
const OVERLAY_VERTICAL_PADDING: f32 = 6.0;
const OVERLAY_FONT_SIZE: f32 = 11.0;
const OVERLAY_NOTE_FONT_SIZE: f32 = 10.0;
const OVERLAY_FONT_FAMILY: &str = "monospace";
const OVERLAY_ROW_GAP: f32 = 12.0;

/// Theme roles supplied by the shell so the overlay follows the active theme.
#[derive(Clone, Copy)]
pub struct PerformancePalette {
    pub surface: u32,
    pub text: u32,
    pub secondary_text: u32,
}

/// Renders the debug performance panel over the page surface.
pub fn performance_overlay(
    diagnostics: &PerformanceDiagnostics,
    palette: PerformancePalette,
) -> impl IntoElement {
    v_stack()
        .absolute()
        .right(px(OVERLAY_INSET))
        .bottom(px(OVERLAY_INSET))
        .gap(px(OVERLAY_GAP))
        .min_w(px(OVERLAY_MIN_WIDTH))
        .px(px(OVERLAY_HORIZONTAL_PADDING))
        .py(px(OVERLAY_VERTICAL_PADDING))
        .bg(rgba(palette.surface))
        .text_size(px(OVERLAY_FONT_SIZE))
        .font_family(OVERLAY_FONT_FAMILY)
        .text_color(rgb(palette.text))
        .child(row(
            "Engine FPS",
            optional_number(diagnostics.frames_per_second, " fps"),
            palette,
        ))
        .child(row(
            "WebContent CPU",
            optional_number(diagnostics.cpu_percent, "%"),
            palette,
        ))
        .child(row(
            "WebContent memory",
            diagnostics
                .memory_bytes
                .map_or_else(|| "—".into(), format_bytes),
            palette,
        ))
        .child(row(
            "Managed heap",
            diagnostics
                .managed_heap_bytes
                .map_or_else(|| "—".into(), format_bytes),
            palette,
        ))
        .child(row(
            "Network ↓ / ↑",
            format!(
                "{} / {}",
                format_rate(diagnostics.download_bytes_per_second),
                format_rate(diagnostics.upload_bytes_per_second)
            ),
            palette,
        ))
        .child(row(
            "Input → next frame",
            optional_number(diagnostics.input_to_frame_latency_ms, " ms"),
            palette,
        ))
        .child(row(
            "Largest frame gap / 10s",
            optional_number(diagnostics.longest_frame_gap_ms, " ms"),
            palette,
        ))
        .child(row("Main-thread task", "not attributed".into(), palette))
        .child(
            div()
                .text_size(px(OVERLAY_NOTE_FONT_SIZE))
                .text_color(rgb(palette.secondary_text))
                .child("Frame gaps are a stall proxy; JS/layout attribution is unavailable."),
        )
        .child(
            div()
                .text_size(px(OVERLAY_NOTE_FONT_SIZE))
                .text_color(rgb(palette.secondary_text))
                .child("Managed heap is the last-GC estimate; some native, media, and GPU memory is missing."),
        )
}

fn row(label: &str, value: String, palette: PerformancePalette) -> impl IntoElement {
    h_stack()
        .items_center()
        .justify_between()
        .gap(px(OVERLAY_ROW_GAP))
        .child(
            div()
                .text_color(rgb(palette.secondary_text))
                .child(label.to_owned()),
        )
        .child(value)
}

fn h_stack() -> Div {
    div().flex().flex_row()
}

fn v_stack() -> Div {
    div().flex().flex_col()
}

fn optional_number(value: Option<f64>, suffix: &str) -> String {
    value.map_or_else(|| "—".into(), |value| format!("{value:.1}{suffix}"))
}

fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1_000.0;
    const MB: f64 = 1_000_000.0;
    const GB: f64 = 1_000_000_000.0;
    let bytes = bytes as f64;
    if bytes >= GB {
        format!("{:.1} GB", bytes / GB)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes / MB)
    } else if bytes >= KB {
        format!("{:.0} KB", bytes / KB)
    } else {
        format!("{bytes:.0} B")
    }
}

fn format_rate(bytes_per_second: u64) -> String {
    format!("{}/s", format_bytes(bytes_per_second))
}
