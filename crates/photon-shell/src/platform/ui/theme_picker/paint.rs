//! Bounded native drawing and coordinate mapping for theme controls.

use super::super::metrics;
use gpui::{
    Bounds, ColorExt, PaintQuad, PathBuilder, Pixels, Point, RenderImage, Window, hsla,
    hsla_to_rgba, point, px, rgb, size,
};
use image::{Frame, RgbaImage};
use photon_core::SpaceTint;
use std::f32::consts::TAU;
use std::{cell::RefCell, collections::VecDeque, sync::Arc};

pub(super) fn tokens() -> &'static crate::platform::theme_config::PickerTokens {
    &crate::platform::theme_config::theme().picker
}
fn tint_color(tint: SpaceTint) -> gpui::Rgba {
    hsla_to_rgba(hsla(
        f32::from(tint.hue % 360) / 360.0,
        f32::from(tint.saturation.min(100)) / 100.0,
        f32::from(tint.lightness.min(100)) / 100.0,
        1.0,
    ))
}
/// Hue is angular; lightness is radial. Positions are normalized, never stored in pixels.
pub(super) fn field_position(tint: SpaceTint) -> (f32, f32) {
    let t = tokens();
    let radius = ((f32::from(tint.lightness.min(100)) / 100.0 - t.field_lightness_min)
        / (t.field_lightness_max - t.field_lightness_min))
        .clamp(0.0, 1.0)
        * 0.5;
    let angle = f32::from(tint.hue % 360) / 360.0 * TAU;
    (0.5 + angle.cos() * radius, 0.5 + angle.sin() * radius)
}
pub(super) fn clamp_field(x: f32, y: f32) -> (f32, f32) {
    let dx = x - 0.5;
    let dy = y - 0.5;
    let scale = (dx.hypot(dy) * 2.0).max(1.0);
    (0.5 + dx / scale, 0.5 + dy / scale)
}
pub(super) fn tint_at(x: f32, y: f32) -> SpaceTint {
    let t = tokens();
    let dx = x - 0.5;
    let dy = y - 0.5;
    let radius = (dx.hypot(dy) * 2.0).min(1.0);
    SpaceTint {
        hue: (dy.atan2(dx).rem_euclid(TAU) / TAU * 360.0).round() as u16 % 360,
        saturation: (t.field_saturation * 100.0).round() as u8,
        lightness: ((t.field_lightness_min
            + (t.field_lightness_max - t.field_lightness_min) * radius)
            * 100.0)
            .round() as u8,
    }
}
pub(super) fn wave_detent(value: u8) -> u8 {
    // Extrema of sin(TAU * cycles * x) occur at quarter-period offsets.
    (f32::from(value) / 100.0 * metrics::PICKER_WAVE_CYCLES * 2.0 + 0.5).floor() as u8
}
pub(super) fn noise_at(x: f32, y: f32) -> Option<u8> {
    let dx = x - 0.5;
    let dy = y - 0.5;
    if dx * dx + dy * dy < 0.04 {
        return None;
    }
    let angle = dx.atan2(-dy).rem_euclid(TAU);
    Some((angle / TAU * f32::from(photon_core::BACKGROUND_NOISE_INTENSITY_MAX)).round() as u8)
}
fn dot(window: &mut Window, center: Point<Pixels>, radius: f32, color: gpui::Rgba) {
    window.paint_quad(
        PaintQuad::new(Bounds::new(
            center - point(px(radius), px(radius)),
            size(px(radius * 2.0), px(radius * 2.0)),
        ))
        .corner_radii(px(radius))
        .background(color),
    );
}
type GridKey = (u32, u32, u32, u32);
type GridCache = VecDeque<(GridKey, Arc<RenderImage>)>;

/// Cache only the decorative dot mask. The marker and all controls remain native geometry.
fn field_grid(bounds: gpui::Size<Pixels>, scale: f32, ink: u32) -> Arc<RenderImage> {
    thread_local! { static CACHE: RefCell<GridCache> = const { RefCell::new(VecDeque::new()) }; }
    let width = (f32::from(bounds.width) * scale).ceil().max(1.0) as u32;
    let height = (f32::from(bounds.height) * scale).ceil().max(1.0) as u32;
    let key = (width, height, scale.to_bits(), ink);
    CACHE.with_borrow_mut(|cache| {
        if let Some((_, image)) = cache.iter().find(|(cached, _)| *cached == key) {
            return image.clone();
        }
        let mut pixels = RgbaImage::new(width, height);
        let spacing = metrics::PICKER_DOT_SPACING * scale;
        let radius = metrics::PICKER_DOT_RADIUS * scale;
        for row in 1..(height as f32 / spacing) as u32 {
            for column in 1..(width as f32 / spacing) as u32 {
                let center_x = column as f32 * spacing;
                let center_y = row as f32 * spacing;
                let extent = radius.ceil() as i32 + 1;
                for dy in -extent..=extent {
                    for dx in -extent..=extent {
                        let x = center_x.floor() as i32 + dx;
                        let y = center_y.floor() as i32 + dy;
                        if x < 0 || y < 0 || x >= width as i32 || y >= height as i32 {
                            continue;
                        }
                        let distance = ((x as f32 + 0.5 - center_x).powi(2)
                            + (y as f32 + 0.5 - center_y).powi(2))
                        .sqrt();
                        let alpha =
                            ((radius + 0.5 - distance).clamp(0.0, 1.0) * tokens().dots * 255.0)
                                .round() as u8;
                        pixels.put_pixel(
                            x as u32,
                            y as u32,
                            image::Rgba([(ink >> 16) as u8, (ink >> 8) as u8, ink as u8, alpha]),
                        );
                    }
                }
            }
        }
        let image = Arc::new(RenderImage::new([Frame::new(pixels)]));
        if cache.len() == 2 {
            cache.pop_front();
        }
        cache.push_back((key, image.clone()));
        image
    })
}

pub(super) struct PaintedHandle {
    pub x: f32,
    pub y: f32,
    pub scale: f32,
    pub opacity: f32,
    pub tint: SpaceTint,
    pub selected: bool,
}
pub(super) fn paint_field(
    bounds: Bounds<Pixels>,
    ink: u32,
    handles: &[PaintedHandle],
    window: &mut Window,
) {
    let t = tokens();
    let width = f32::from(bounds.size.width);
    let corner = metrics::theme_accent_panel_radius();
    window.paint_quad(
        PaintQuad::new(bounds)
            .corner_radii(px(corner))
            .background(rgb(ink).opacity(t.field_surface)),
    );
    let texture = field_grid(bounds.size, window.scale_factor(), ink);
    window
        .paint_image(bounds, bounds, px(corner).into(), 0.0, texture, 0, false)
        .ok();
    let inset = metrics::PICKER_HANDLE_RADIUS;
    for handle in handles {
        let radius = inset * handle.scale;
        if radius < 0.1 || handle.opacity < 0.01 {
            continue;
        }
        let center = bounds.origin
            + point(
                px(inset + handle.x.clamp(0.0, 1.0) * (width - 2.0 * inset)),
                px(inset
                    + handle.y.clamp(0.0, 1.0) * (f32::from(bounds.size.height) - 2.0 * inset)),
            );
        dot(window, center, radius, rgb(ink).opacity(handle.opacity));
        dot(
            window,
            center,
            (radius - metrics::PICKER_HANDLE_BORDER * handle.scale).max(0.0),
            tint_color(handle.tint).opacity(handle.opacity),
        );
    }
}

fn wave_point(origin: Point<Pixels>, width: f32, amplitude: f32, x: f32) -> Point<Pixels> {
    origin
        + point(
            px(x * width),
            px(-(x * TAU * metrics::PICKER_WAVE_CYCLES).sin() * amplitude),
        )
}

/// A single filled outline keeps round caps at the same opacity as the stroke.
fn wave_outline(
    origin: Point<Pixels>,
    width: f32,
    amplitude: f32,
    end: f32,
) -> Option<gpui::Path<Pixels>> {
    if end <= 0.0 {
        return None;
    }
    let radius = metrics::PICKER_WAVE_STROKE / 2.0;
    let tangent = |x: f32| {
        (-amplitude
            * TAU
            * metrics::PICKER_WAVE_CYCLES
            * (x * TAU * metrics::PICKER_WAVE_CYCLES).cos())
        .atan2(width)
    };
    let offset = |angle: f32| point(px(angle.cos() * radius), px(angle.sin() * radius));
    let mut outline = PathBuilder::fill();
    let samples = metrics::PICKER_WAVE_SAMPLES;
    for sample in 0..=samples {
        let x = sample as f32 / samples as f32 * end;
        let position = wave_point(origin, width, amplitude, x)
            + offset(tangent(x) + std::f32::consts::FRAC_PI_2);
        if sample == 0 {
            outline.move_to(position);
        } else {
            outline.line_to(position);
        }
    }
    let arc_samples = metrics::PICKER_WAVE_CAP_SAMPLES;
    let last = wave_point(origin, width, amplitude, end);
    for sample in 1..=arc_samples {
        let angle = tangent(end) + std::f32::consts::FRAC_PI_2
            - std::f32::consts::PI * sample as f32 / arc_samples as f32;
        outline.line_to(last + offset(angle));
    }
    for sample in (0..samples).rev() {
        let x = sample as f32 / samples as f32 * end;
        outline.line_to(
            wave_point(origin, width, amplitude, x)
                + offset(tangent(x) - std::f32::consts::FRAC_PI_2),
        );
    }
    for sample in 1..=arc_samples {
        let angle = tangent(0.0)
            - std::f32::consts::FRAC_PI_2
            - std::f32::consts::PI * sample as f32 / arc_samples as f32;
        outline.line_to(origin + offset(angle));
    }
    outline.close();
    outline.build().ok()
}

pub(super) fn wave_track(bounds: Bounds<Pixels>) -> Bounds<Pixels> {
    Bounds::new(
        point(
            bounds.origin.x,
            bounds.center().y - px(metrics::PICKER_TRACK_WIDTH / 2.0),
        ),
        size(bounds.size.width, px(metrics::PICKER_TRACK_WIDTH)),
    )
}

pub(super) fn paint_wave(
    bounds: Bounds<Pixels>,
    value: f32,
    ink: u32,
    enabled: bool,
    window: &mut Window,
) {
    let fraction = value.clamp(0.0, 1.0);
    let inset = metrics::PICKER_THUMB_MAX_WIDTH / 2.0;
    let width = (f32::from(bounds.size.width) - inset * 2.0).max(1.0);
    let center_y = bounds.center().y;
    let origin = point(bounds.origin.x + px(inset), center_y);
    let amplitude = metrics::PICKER_WAVE_AMPLITUDE * if enabled { fraction } else { 1.0 };
    window.paint_quad(
        PaintQuad::new(wave_track(bounds))
            .corner_radii(px(metrics::PICKER_TRACK_WIDTH / 2.0))
            .background(rgb(ink).opacity(tokens().track)),
    );
    if let Some(path) = wave_outline(origin, width, amplitude, 1.0) {
        window.paint_path(path, rgb(ink).opacity(tokens().inactive));
    }
    if !enabled {
        return;
    }
    if let Some(path) = wave_outline(origin, width, amplitude, fraction) {
        window.paint_path(path, rgb(ink));
    }
    let thumb_width = metrics::PICKER_THUMB_MIN_WIDTH
        + (metrics::PICKER_THUMB_MAX_WIDTH - metrics::PICKER_THUMB_MIN_WIDTH) * fraction;
    let thumb_height = metrics::PICKER_THUMB_MIN_HEIGHT
        + (metrics::PICKER_THUMB_MAX_HEIGHT - metrics::PICKER_THUMB_MIN_HEIGHT) * fraction;
    window.paint_quad(
        PaintQuad::new(Bounds::new(
            point(
                origin.x + px(width * fraction - thumb_width / 2.0),
                center_y - px(thumb_height / 2.0),
            ),
            size(px(thumb_width), px(thumb_height)),
        ))
        .corner_radii(px(thumb_width / 2.0))
        .background(rgb(ink)),
    );
}
pub(super) fn paint_knob(
    bounds: Bounds<Pixels>,
    noise: u8,
    ink: u32,
    enabled: bool,
    window: &mut Window,
) {
    let center = bounds.center();
    let radius = f32::from(bounds.size.width) / 2.0 - metrics::PICKER_KNOB_DOT * 2.0;
    let count = photon_core::BACKGROUND_NOISE_INTENSITY_MAX;
    for notch in 0..count {
        let angle = f32::from(notch) / f32::from(count) * TAU;
        dot(
            window,
            center + point(px(angle.sin() * radius), px(-angle.cos() * radius)),
            metrics::PICKER_KNOB_DOT,
            rgb(ink).opacity(tokens().inactive),
        );
    }
    window.paint_quad(
        PaintQuad::new(Bounds::new(
            center - point(px(radius * 0.65), px(radius * 0.65)),
            size(px(radius * 1.3), px(radius * 1.3)),
        ))
        .corner_radii(px(radius * 0.65))
        .border_widths(px(metrics::SURFACE_BORDER_WIDTH))
        .border_color(rgb(ink).opacity(tokens().track)),
    );
    if !enabled {
        return;
    }
    let angle = f32::from(noise.min(count)) / f32::from(count) * TAU;
    let handle = center + point(px(angle.sin() * radius), px(-angle.cos() * radius));
    let half_length = metrics::PICKER_KNOB_DOT * 1.4;
    let offset = point(
        px(angle.sin() * half_length),
        px(-angle.cos() * half_length),
    );
    let mut indicator = PathBuilder::stroke(px(metrics::PICKER_KNOB_DOT * 2.0));
    indicator.move_to(handle - offset);
    indicator.line_to(handle + offset);
    if let Ok(path) = indicator.build() {
        window.paint_path(path, rgb(ink));
    }
    dot(window, handle - offset, metrics::PICKER_KNOB_DOT, rgb(ink));
    dot(window, handle + offset, metrics::PICKER_KNOB_DOT, rgb(ink));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn round_wave_outline_extends_past_both_endpoints() {
        let radius = metrics::PICKER_WAVE_STROKE / 2.0;
        let path = wave_outline(point(px(10.0), px(20.0)), 100.0, 0.0, 1.0).unwrap();
        assert!((f32::from(path.bounds.left()) - (10.0 - radius)).abs() < 0.01);
        assert!((f32::from(path.bounds.right()) - (110.0 + radius)).abs() < 0.01);
        assert_eq!(
            f32::from(path.bounds.size.height),
            metrics::PICKER_WAVE_STROKE
        );
    }
    #[test]
    fn track_height_stays_fixed_while_the_wave_grows() {
        let bounds = Bounds::new(
            point(px(0.0), px(0.0)),
            size(px(220.0), px(metrics::PICKER_FOOTER_HEIGHT)),
        );
        let track = wave_track(bounds);
        assert_eq!(f32::from(track.size.height), metrics::PICKER_TRACK_WIDTH);
        let flat = wave_outline(bounds.center(), 200.0, 0.0, 1.0).unwrap();
        let wave =
            wave_outline(bounds.center(), 200.0, metrics::PICKER_WAVE_AMPLITUDE, 1.0).unwrap();
        assert!(wave.bounds.size.height > flat.bounds.size.height);
        assert_eq!(wave_track(bounds), track);
    }
}
