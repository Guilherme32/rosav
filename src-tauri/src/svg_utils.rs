use itertools::Itertools;

pub struct GraphLimits {
    pub x: (f64, f64),
    pub y: (f64, f64),
}

pub fn convert_point(
    graph_limits: &GraphLimits,
    svg_limits: &(f64, f64),
    og_point: &(f64, f64),
) -> (f64, f64) {
    let limits_y = (graph_limits.y.1, graph_limits.y.0); // Invert because svg coords
    let limits_x = graph_limits.x;

    let x = (og_point.0 - limits_x.0) / (limits_x.1 - limits_x.0);
    let x = x * svg_limits.0;

    let y = (og_point.1 - limits_y.0) / (limits_y.1 - limits_y.0);
    let y = y * svg_limits.1;

    (x, y)
}

// TODO Beziers have a shorthand for multiple points. Maybe try it and see if
// it gives some performance increase (Maybe check resize fps and cpu usage)
pub fn bezier_point(
    previous: (f64, f64),
    start: (f64, f64),
    end: (f64, f64),
    next: (f64, f64),
) -> String {
    let smoothing = 0.3;

    let start_vector = (end.0 - previous.0, end.1 - previous.1);
    let start_control = (
        start.0 + start_vector.0 * smoothing,
        start.1 + start_vector.1 * smoothing,
    );

    let end_vector = (start.0 - next.0, start.1 - next.1);
    let end_control = (
        end.0 + end_vector.0 * smoothing,
        end.1 + end_vector.1 * smoothing,
    );

    format!(
        "C {:.2},{:.2} {:.2},{:.2}, {:.2},{:.2} ",
        start_control.0, start_control.1, end_control.0, end_control.1, end.0, end.1
    )
}

pub fn bezier_path(
    points: &[(f64, f64)],
    svg_limits: (u32, u32),
    graph_limits: &GraphLimits,
) -> String {
    let svg_limits = (svg_limits.0 as f64 - 40.0, svg_limits.1 as f64 - 16.6);

    if points.is_empty() {
        return "".to_string();
    }

    let cvt = |point| convert_point(graph_limits, &svg_limits, point);
    let start = cvt(&points[0]);
    let start = format!("M {:.2},{:.2} ", start.0, start.1);

    let last_entry = points.last().unwrap(); // The size is checked above

    let path = points
        .iter()
        .chain((0..3).map(|_| last_entry)) // Without this the end is cropped
        .map(cvt)
        .tuple_windows() // Cropped because of the window
        .map(|(a, b, c, d)| bezier_point(a, b, c, d))
        .collect::<String>();
    let path = format!("{start}{path}");

    path
}
