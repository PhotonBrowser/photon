//! The copy link icon: a link that shrinks away when pressed, a tick that
//! pops up in its place and draws its stroke on, and the link returning once
//! the tick has been seen.
//!
//! One clock drives every part, so nothing restarts or swaps mid-frame.

use gpui::{
    App, Bounds, PathBuilder, Pixels, Point, Transformation, Window, canvas, div, point,
    prelude::*, px, rgb, size,
};
use std::time::{Duration, Instant};

use super::icons::{icon, lucide};
use super::metrics;

/// The tick's corners in Lucide's 24-unit grid: from the short arm's end,
/// down to the corner, up to the long arm's end.
const TICK: [(f32, f32); 3] = [(4.0, 12.0), (9.0, 17.0), (20.0, 6.0)];
const GRID: f32 = 24.0;
const STROKE: f32 = 2.0;

/// The icon `copied_at` into its copy, drawn `size` square in `color`.
/// `None` rests on the link. Without motion it snaps between the two.
/// While moving it asks for the next frame itself.
pub(super) fn copy_link_icon(
    copied_at: Option<Instant>,
    allows_motion: bool,
    color: u32,
    icon_size: f32,
) -> impl IntoElement {
    let frame = match copied_at {
        Some(at) if allows_motion => Frame::at(at.elapsed()),
        Some(_) => Frame::SETTLED_COPIED,
        None => Frame::REST,
    };
    let link = icon(lucide!("link"), color, icon_size).with_transformation(Transformation::scale(
        size(frame.link_scale, frame.link_scale),
    ));
    div()
        .relative()
        .size(px(icon_size))
        .flex_none()
        .child(
            div()
                .absolute()
                .inset_0()
                .opacity(frame.link_opacity)
                .child(link),
        )
        .child(
            // The canvas also keeps the clock ticking until the motion ends.
            canvas(
                |_, _, _| {},
                move |bounds, _, window: &mut Window, _: &mut App| {
                    if frame.animating {
                        window.request_animation_frame();
                    }
                    paint_tick(window, bounds, frame, color);
                },
            )
            .absolute()
            .inset_0(),
        )
}

/// Where each part of the icon is at one moment.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Frame {
    link_scale: f32,
    link_opacity: f32,
    tick_scale: f32,
    tick_opacity: f32,
    /// How much of the tick's stroke is drawn, from its short arm.
    tick_drawn: f32,
    animating: bool,
}

impl Frame {
    const REST: Self = Self {
        link_scale: 1.0,
        link_opacity: 1.0,
        tick_scale: 1.0,
        tick_opacity: 0.0,
        tick_drawn: 0.0,
        animating: false,
    };
    const SETTLED_COPIED: Self = Self {
        link_scale: 1.0,
        link_opacity: 0.0,
        tick_scale: 1.0,
        tick_opacity: 1.0,
        tick_drawn: 1.0,
        animating: false,
    };

    fn at(elapsed: Duration) -> Self {
        let ms = elapsed.as_secs_f32() * 1000.0;
        let phase = |start: u64, length: u64| ((ms - start as f32) / length as f32).clamp(0.0, 1.0);

        let shrink = ease_in(phase(0, metrics::COPY_ICON_SHRINK_MS));
        let pop = phase(metrics::COPY_ICON_POP_DELAY_MS, metrics::COPY_ICON_POP_MS);
        let draw = ease_out(phase(
            metrics::COPY_ICON_DRAW_DELAY_MS,
            metrics::COPY_ICON_DRAW_MS,
        ));
        let leave = phase(metrics::COPY_LINK_CONFIRM_MS, metrics::COPY_ICON_RETURN_MS);
        let end = metrics::COPY_LINK_CONFIRM_MS + metrics::COPY_ICON_RETURN_MS;

        let hidden = metrics::COPY_ICON_HIDDEN_SCALE;
        let returning = back_out(leave);
        Self {
            link_scale: if leave > 0.0 {
                mix(hidden, 1.0, returning)
            } else {
                mix(1.0, hidden, shrink)
            },
            link_opacity: if leave > 0.0 { leave } else { 1.0 - shrink },
            tick_scale: if leave > 0.0 {
                mix(1.0, hidden, ease_in(leave))
            } else {
                mix(hidden, 1.0, back_out(pop))
            },
            tick_opacity: if leave > 0.0 {
                1.0 - leave
            } else {
                pop.min(1.0)
            },
            tick_drawn: draw,
            animating: ms < end as f32,
        }
    }
}

/// Strokes the drawn share of the tick, scaled about the icon's centre, with
/// round ends and corner as Lucide draws it.
fn paint_tick(window: &mut Window, bounds: Bounds<Pixels>, frame: Frame, color: u32) {
    if frame.tick_drawn <= 0.0 {
        return;
    }
    let unit = f32::from(bounds.size.width) / GRID * frame.tick_scale;
    let center = bounds.center();
    let at = |(x, y): (f32, f32)| {
        center + point(px((x - GRID / 2.0) * unit), px((y - GRID / 2.0) * unit))
    };
    let [start, corner, end] = TICK.map(at);
    let short = distance(start, corner);
    let drawn = frame.tick_drawn * (short + distance(corner, end));
    let tip = if drawn <= short {
        lerp(start, corner, drawn / short)
    } else {
        lerp(corner, end, (drawn - short) / distance(corner, end))
    };

    let width = STROKE * unit;
    let color = gpui::ColorExt::opacity(&rgb(color), frame.tick_opacity);
    let mut path = PathBuilder::stroke(px(width));
    path.move_to(start);
    if drawn > short {
        path.line_to(corner);
    }
    path.line_to(tip);
    if let Ok(path) = path.build() {
        window.paint_path(path, color);
    }
    let caps = [Some(start), (drawn > short).then_some(corner), Some(tip)];
    for cap in caps.into_iter().flatten() {
        paint_dot(window, cap, width / 2.0, color);
    }
}

fn paint_dot(window: &mut Window, center: Point<Pixels>, radius: f32, color: gpui::Rgba) {
    let r = point(px(radius), px(radius));
    let mut dot = PathBuilder::fill();
    dot.move_to(center - point(px(radius), px(0.0)));
    dot.arc_to(r, px(0.0), false, true, center + point(px(radius), px(0.0)));
    dot.arc_to(r, px(0.0), false, true, center - point(px(radius), px(0.0)));
    dot.close();
    if let Ok(dot) = dot.build() {
        window.paint_path(dot, color);
    }
}

fn distance(a: Point<Pixels>, b: Point<Pixels>) -> f32 {
    f32::from(a.x - b.x).hypot(f32::from(a.y - b.y))
}

fn lerp(a: Point<Pixels>, b: Point<Pixels>, t: f32) -> Point<Pixels> {
    a + point((b.x - a.x) * t, (b.y - a.y) * t)
}

fn mix(from: f32, to: f32, t: f32) -> f32 {
    from + (to - from) * t
}

fn ease_in(t: f32) -> f32 {
    t * t * t
}

fn ease_out(t: f32) -> f32 {
    1.0 - (1.0 - t).powi(3)
}

/// Passes its end a little and settles back, for a pop.
fn back_out(t: f32) -> f32 {
    let overshoot = metrics::COPY_ICON_OVERSHOOT;
    let t = t - 1.0;
    1.0 + (overshoot + 1.0) * t.powi(3) + overshoot * t.powi(2)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_on_the_link() {
        let frame = Frame::at(Duration::ZERO);
        assert_eq!(frame.link_opacity, 1.0);
        assert_eq!(frame.tick_drawn, 0.0);
        assert!(frame.animating);
    }

    #[test]
    fn holds_the_drawn_tick() {
        let frame = Frame::at(Duration::from_millis(metrics::COPY_LINK_CONFIRM_MS - 1));
        assert_eq!(frame.link_opacity, 0.0);
        assert_eq!(frame.tick_opacity, 1.0);
        assert_eq!(frame.tick_drawn, 1.0);
        assert!((frame.tick_scale - 1.0).abs() < 1e-4);
    }

    #[test]
    fn returns_to_the_link_and_stops() {
        let end = metrics::COPY_LINK_CONFIRM_MS + metrics::COPY_ICON_RETURN_MS;
        let frame = Frame::at(Duration::from_millis(end));
        assert_eq!(frame.link_opacity, 1.0);
        assert_eq!(frame.tick_opacity, 0.0);
        assert!((frame.link_scale - 1.0).abs() < 1e-4);
        assert!(!frame.animating);
    }
}
