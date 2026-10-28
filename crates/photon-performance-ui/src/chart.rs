use gpui::{Bounds, Canvas, PaintQuad, Pixels, Styled, Window, canvas, point, px, rgba, size};

const GRID_DIVISIONS: usize = 3;
const GRID_LINE_WIDTH: f32 = 1.0;
const SERIES_LINE_WIDTH: f32 = 1.75;
const SERIES_SAMPLE_STEP: f32 = 1.0;

/// A value positioned in a rolling timeline, with `None` marking a gap.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct ChartPoint {
    pub elapsed_seconds: f64,
    pub value: Option<f64>,
}

/// Draws a lightweight, theme-colored line chart in the space its parent gives it.
pub fn performance_chart(points: Vec<ChartPoint>, line_color: u32, grid_color: u32) -> Canvas<()> {
    canvas(
        |_bounds, _window, _cx| (),
        move |bounds, (), window, _cx| {
            let left = bounds.origin.x + px(1.0);
            let top = bounds.origin.y + px(2.0);
            let width = (bounds.size.width - px(2.0)).max(px(0.0));
            let height = (bounds.size.height - px(4.0)).max(px(0.0));
            if width == px(0.0) || height == px(0.0) {
                return;
            }

            for row in 0..=GRID_DIVISIONS {
                let y = top + height * (row as f32 / GRID_DIVISIONS as f32);
                window.paint_quad(
                    PaintQuad::new(Bounds::new(
                        point(left, y - px(GRID_LINE_WIDTH / 2.0)),
                        size(width, px(GRID_LINE_WIDTH)),
                    ))
                    .background(rgba(grid_color)),
                );
            }
            for column in 1..=GRID_DIVISIONS {
                let x = left + width * (column as f32 / (GRID_DIVISIONS + 1) as f32);
                window.paint_quad(
                    PaintQuad::new(Bounds::new(
                        point(x - px(GRID_LINE_WIDTH / 2.0), top),
                        size(px(GRID_LINE_WIDTH), height),
                    ))
                    .background(rgba(grid_color)),
                );
            }

            draw_series(points, left, top, width, height, line_color, window);
        },
    )
    .size_full()
}

fn draw_series(
    points: Vec<ChartPoint>,
    left: Pixels,
    top: Pixels,
    width: Pixels,
    height: Pixels,
    color: u32,
    window: &mut Window,
) {
    let Some(first) = points.first() else {
        return;
    };
    let Some(last) = points.last() else {
        return;
    };
    let first_time = first.elapsed_seconds;
    let time_span = (last.elapsed_seconds - first_time).max(0.001);
    let value_max = points
        .iter()
        .filter_map(|point| point.value)
        .filter(|value| value.is_finite())
        .fold(0.0_f64, f64::max)
        .max(1.0);

    let mut previous = None;
    for sample in points {
        let Some(value) = sample.value.filter(|value| value.is_finite()) else {
            previous = None;
            continue;
        };
        let time_fraction = ((sample.elapsed_seconds - first_time) / time_span).clamp(0.0, 1.0);
        let value_fraction = (value / value_max).clamp(0.0, 1.0);
        let current = point(
            left + width * time_fraction as f32,
            top + height * (1.0 - value_fraction as f32),
        );
        if let Some(previous) = previous {
            draw_segment(previous, current, color, window);
        }
        previous = Some(current);
    }
}

/// Paints a line with overlapping quads so charts do not allocate GPUI's
/// full-window path-rasterization texture.
fn draw_segment(
    start: gpui::Point<Pixels>,
    end: gpui::Point<Pixels>,
    color: u32,
    window: &mut Window,
) {
    let dx = end.x - start.x;
    let dy = end.y - start.y;
    let steps = (dx.as_f32().abs().max(dy.as_f32().abs()) / SERIES_SAMPLE_STEP)
        .ceil()
        .max(1.0) as usize;
    let diameter = px(SERIES_LINE_WIDTH);

    for step in 0..=steps {
        let progress = step as f32 / steps as f32;
        let center = point(start.x + dx * progress, start.y + dy * progress);
        window.paint_quad(
            PaintQuad::new(Bounds::new(
                point(center.x - diameter / 2.0, center.y - diameter / 2.0),
                size(diameter, diameter),
            ))
            .corner_radii(diameter / 2.0)
            .background(rgba(color)),
        );
    }
}
