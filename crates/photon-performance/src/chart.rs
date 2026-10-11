use gpui::{Canvas, PathBuilder, Pixels, Styled, canvas, point, px, rgba};

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

            let mut grid = PathBuilder::stroke(px(1.0));
            for row in 0..=3 {
                let y = top + height * (row as f32 / 3.0);
                grid.move_to(point(left, y));
                grid.line_to(point(left + width, y));
            }
            for column in 1..=3 {
                let x = left + width * (column as f32 / 4.0);
                grid.move_to(point(x, top));
                grid.line_to(point(x, top + height));
            }
            if let Ok(grid) = grid.build() {
                window.paint_path(grid, rgba(grid_color));
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
    window: &mut gpui::Window,
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

    let mut line = PathBuilder::stroke(px(1.75));
    let mut segment_started = false;
    for sample in points {
        let Some(value) = sample.value.filter(|value| value.is_finite()) else {
            segment_started = false;
            continue;
        };
        let time_fraction = ((sample.elapsed_seconds - first_time) / time_span).clamp(0.0, 1.0);
        let value_fraction = (value / value_max).clamp(0.0, 1.0);
        let x = left + width * time_fraction as f32;
        let y = top + height * (1.0 - value_fraction as f32);
        if segment_started {
            line.line_to(point(x, y));
        } else {
            line.move_to(point(x, y));
        }
        segment_started = true;
    }
    if let Ok(line) = line.build() {
        window.paint_path(line, rgba(color));
    }
}
