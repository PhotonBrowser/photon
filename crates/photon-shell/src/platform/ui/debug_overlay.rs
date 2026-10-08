//! Plain diagnostics text over the page surface.

use gpui::{div, prelude::*, px, rgb, rgba};
use photon_core::BrowserDiagnostics;

use super::layout::{h_stack, v_stack};
use super::theme::Palette;

pub(super) fn debug_overlay(
    diagnostics: &BrowserDiagnostics,
    palette: Palette,
) -> impl IntoElement {
    v_stack()
        .absolute()
        .right(px(12.0))
        .bottom(px(12.0))
        .gap(px(2.0))
        .min_w(px(236.0))
        .px(px(8.0))
        .py(px(6.0))
        .bg(rgba(palette.diagnostics_surface))
        .text_size(px(11.0))
        .font_family("monospace")
        .text_color(rgb(palette.diagnostics_text))
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
                .text_size(px(10.0))
                .text_color(rgb(palette.diagnostics_muted))
                .child("Frame gaps are a stall proxy; JS/layout attribution is unavailable."),
        )
}

fn row(label: &str, value: String, palette: Palette) -> impl IntoElement {
    h_stack()
        .items_center()
        .justify_between()
        .gap(px(12.0))
        .child(
            div()
                .text_color(rgb(palette.diagnostics_muted))
                .child(label.to_owned()),
        )
        .child(value)
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
