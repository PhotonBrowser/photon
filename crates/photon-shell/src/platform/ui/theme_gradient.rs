//! Three-stop gradients drawn with bounded native paths on GPUI's two-stop shader.

use gpui::{
    Bounds, PathBuilder, Pixels, Point, Window, linear_color_stop, linear_gradient, point, px,
    rgb_to_hsla, rgba,
};

pub(super) fn paint(
    bounds: Bounds<Pixels>,
    colors: [u32; 3],
    angle: f32,
    radius: f32,
    window: &mut Window,
) {
    let radians = angle.to_radians();
    let axis = (radians.sin(), -radians.cos());
    let center = bounds.center();
    let project = |point: Point<Pixels>| {
        f32::from(point.x - center.x) * axis.0 + f32::from(point.y - center.y) * axis.1
    };
    let extent = (axis.0.abs() * f32::from(bounds.size.width)
        + axis.1.abs() * f32::from(bounds.size.height))
        / 2.0;
    let rectangle = rounded_rectangle(bounds, radius);
    for half in 0..2 {
        let polygon = clip(&rectangle, axis, center, half == 0);
        if polygon.len() < 3 {
            continue;
        }
        let mut builder = PathBuilder::fill();
        builder.move_to(polygon[0]);
        for vertex in &polygon[1..] {
            builder.line_to(*vertex);
        }
        builder.close();
        let Ok(path) = builder.build() else {
            continue;
        };
        let half_extent = (axis.0.abs() * f32::from(path.bounds.size.width)
            + axis.1.abs() * f32::from(path.bounds.size.height))
            / 2.0;
        if half_extent <= f32::EPSILON {
            continue;
        }
        let offset = project(path.bounds.center());
        let start = if half == 0 { -extent } else { 0.0 };
        let end = if half == 0 { 0.0 } else { extent };
        let stop = |projection: f32| {
            ((projection - offset + half_extent) / (2.0 * half_extent)).clamp(0.0, 1.0)
        };
        window.paint_path(
            path,
            linear_gradient(
                angle,
                linear_color_stop(rgb_to_hsla(rgba(colors[half])), stop(start)),
                linear_color_stop(rgb_to_hsla(rgba(colors[half + 1])), stop(end)),
            ),
        );
    }
}

fn rounded_rectangle(bounds: Bounds<Pixels>, radius: f32) -> Vec<Point<Pixels>> {
    let radius = radius.clamp(
        0.0,
        f32::from(bounds.size.width.min(bounds.size.height)) / 2.0,
    );
    if radius <= f32::EPSILON {
        return vec![
            bounds.origin,
            point(bounds.right(), bounds.top()),
            point(bounds.right(), bounds.bottom()),
            point(bounds.left(), bounds.bottom()),
        ];
    }
    let radius_px = px(radius);
    let corners = [
        point(bounds.right() - radius_px, bounds.top() + radius_px),
        point(bounds.right() - radius_px, bounds.bottom() - radius_px),
        point(bounds.left() + radius_px, bounds.bottom() - radius_px),
        point(bounds.left() + radius_px, bounds.top() + radius_px),
    ];
    // Eight arc segments stay within a fraction of a pixel at shell corner sizes.
    corners
        .into_iter()
        .enumerate()
        .flat_map(|(corner, center)| {
            (0..=8).map(move |step| {
                let angle =
                    (-1.0 + corner as f32 + step as f32 / 8.0) * std::f32::consts::FRAC_PI_2;
                center + point(px(angle.cos() * radius), px(angle.sin() * radius))
            })
        })
        .collect()
}

fn clip(
    rectangle: &[Point<Pixels>],
    axis: (f32, f32),
    center: Point<Pixels>,
    first: bool,
) -> Vec<Point<Pixels>> {
    let distance = |p: Point<Pixels>| {
        (f32::from(p.x - center.x) * axis.0 + f32::from(p.y - center.y) * axis.1)
            * if first { 1.0 } else { -1.0 }
    };
    let mut result = Vec::with_capacity(5);
    for index in 0..rectangle.len() {
        let current = rectangle[index];
        let next = rectangle[(index + 1) % rectangle.len()];
        let a = distance(current);
        let b = distance(next);
        if a <= 0.0 {
            result.push(current);
        }
        if (a <= 0.0) != (b <= 0.0) {
            let t = a / (a - b);
            result.push(point(
                current.x + px(f32::from(next.x - current.x) * t),
                current.y + px(f32::from(next.y - current.y) * t),
            ));
        }
    }
    result.dedup();
    if result.len() > 1 && result.first() == result.last() {
        result.pop();
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn diagonal_halves_cover_the_rectangle_without_losing_the_middle_colour() {
        let rectangle = [
            point(px(0.0), px(0.0)),
            point(px(300.0), px(0.0)),
            point(px(300.0), px(200.0)),
            point(px(0.0), px(200.0)),
        ];
        let center = point(px(150.0), px(100.0));
        let axis = (0.70710677, 0.70710677);
        let area = |polygon: &[Point<Pixels>]| {
            (0..polygon.len())
                .map(|i| {
                    let a = polygon[i];
                    let b = polygon[(i + 1) % polygon.len()];
                    f32::from(a.x) * f32::from(b.y) - f32::from(a.y) * f32::from(b.x)
                })
                .sum::<f32>()
                .abs()
                / 2.0
        };
        let first = clip(&rectangle, axis, center, true);
        let second = clip(&rectangle, axis, center, false);
        assert!((area(&first) + area(&second) - 60_000.0).abs() < 0.1);
    }
}
