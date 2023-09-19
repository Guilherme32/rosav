use gloo_timers::future::TimeoutFuture;
use sycamore::futures::spawn_local_scoped;
use sycamore::prelude::*;

use crate::api::*;
use crate::js_glue::*;
use crate::trace::*;

fn is_inside_graph(
    coordinates: (i32, i32),
    limits_x: (i32, i32),
    limits_y: (i32, i32),
    expand_limits: bool,
) -> bool {
    if expand_limits {
        ((limits_x.0 - 40)..(limits_x.1 + 40)).contains(&coordinates.0)
            && ((limits_y.0 - 40)..(limits_y.1 + 40)).contains(&coordinates.1)
    } else {
        (limits_x.0..limits_x.1).contains(&coordinates.0)
            && (limits_y.0..limits_y.1).contains(&coordinates.1)
    }
}

fn global_to_graph_pixel(coordinates: (i32, i32)) -> (i32, i32) {
    (coordinates.0 - 214, coordinates.1 - 10)
}

fn clip_to_graph(local_coordinates: (i32, i32), graph_size: (i32, i32)) -> (i32, i32) {
    let x = if local_coordinates.0 < 2 {
        2
    } else if local_coordinates.0 > graph_size.0 - 2 {
        graph_size.0 - 2
    } else {
        local_coordinates.0
    };

    let y = if local_coordinates.1 < 2 {
        2
    } else if local_coordinates.1 > graph_size.1 - 2 {
        graph_size.1 - 2
    } else {
        local_coordinates.1
    };

    (x, y)
}

fn svg_to_graph_size(svg_size: (i32, i32)) -> (i32, i32) {
    (
        svg_size.0 - 40, // 32 e 16 para os labels dos eixos
        svg_size.1 - 16,
    )
}

fn fix_positions(
    start_position: (i32, i32),
    end_position: (i32, i32),
    svg_size: (i32, i32),
) -> ((i32, i32), (i32, i32)) {
    let (mut start_x, mut start_y) = global_to_graph_pixel(start_position);
    let (end_x, end_y) = global_to_graph_pixel(end_position);

    let graph_size = svg_to_graph_size(svg_size);
    let (mut end_x, mut end_y) = clip_to_graph((end_x, end_y), graph_size);

    if start_x > end_x {
        (start_x, end_x) = (end_x, start_x);
    }
    if start_y > end_y {
        (start_y, end_y) = (end_y, start_y);
    }

    ((start_x, start_y), (end_x, end_y))
}

fn graph_to_spectrum_point(
    graph_point: (i32, i32),
    svg_size: (i32, i32),
    power_limits: (f64, f64),
    wavelength_limits: (f64, f64),
) -> (f64, f64) {
    let graph_size = svg_to_graph_size(svg_size);

    let graph_x = graph_point.0 as f64;
    let x_t = graph_x / (graph_size.0 as f64);
    let wavelength = wavelength_limits.0 + x_t * (wavelength_limits.1 - wavelength_limits.0);

    let graph_y = graph_point.1 as f64;
    let y_t = graph_y / (graph_size.1 as f64);
    let power = power_limits.0 + y_t * (power_limits.1 - power_limits.0);

    (wavelength, power)
}

fn traces_have_spectrum(traces: &ReadSignal<Vec<Trace>>) -> bool {
    for trace in &*traces.get() {
        if !trace.svg_path.is_empty() {
            return true;
        }
    }

    false
}

#[derive(Prop)]
pub struct GraphProps<'a> {
    svg_size: &'a ReadSignal<(i32, i32)>,
    traces: &'a ReadSignal<Vec<Trace>>,
    shadow_paths: &'a ReadSignal<Vec<String>>,
    draw_shadow: &'a ReadSignal<bool>,
    time_series_paths: &'a ReadSignal<TimeSeriesGroupPaths>,
    draw_time_series: &'a ReadSignal<bool>,
    limits_change_flag: &'a Signal<bool>,
}

#[component]
pub fn Graph<'a, G: Html>(cx: Scope<'a>, props: GraphProps<'a>) -> View<G> {
    let selecting = create_signal(cx, false);
    let pointer_down = create_signal(cx, false);
    let starting_position = create_signal(cx, (0, 0));
    let position = create_signal(cx, (0, 0));

    let graph_x_bounds = create_memo(cx, || {
        let min = 210;
        let max = min + props.svg_size.get().0 - 40;
        (min, max)
    });
    let graph_y_bounds = create_memo(cx, || {
        let min = 5;
        let max = min + props.svg_size.get().1 - 16;
        (min, max)
    });

    // Start zoom behaviour (TODO make this prettier on the code side)
    spawn_local_scoped(cx, async move {
        loop {
            wait_for_pointer_up().await;
            pointer_down.set(false);
            if *selecting.get() && traces_have_spectrum(props.traces) {
                let ((start_x, start_y), (end_x, end_y)) = fix_positions(
                    *starting_position.get(),
                    *position.get(),
                    *props.svg_size.get(),
                );

                let height = end_y - start_y;
                let width = end_x - start_x;
                if width < 10 || height < 10 {
                    // Prevent infinitely small zooms
                    selecting.set(false);
                    continue;
                }

                let new_min = graph_to_spectrum_point(
                    (start_x, end_y), // SVG space is inverted on the y axis
                    *props.svg_size.get(),
                    get_power_limits().await,
                    get_wavelength_limits().await,
                );
                let new_max = graph_to_spectrum_point(
                    (end_x, start_y),
                    *props.svg_size.get(),
                    get_power_limits().await,
                    get_wavelength_limits().await,
                );
                let wavelength_limits = (new_min.0, new_max.0);
                let power_limits = (new_min.1, new_max.1);
                change_limits(Some(wavelength_limits), Some(power_limits)).await;

                props.limits_change_flag.set(true);
            }
            selecting.set(false);
        }
    });

    spawn_local_scoped(cx, async move {
        loop {
            wait_for_pointer_down().await;
            pointer_down.set(true);

            let new_position = get_pointer_position();
            if is_inside_graph(
                new_position,
                *graph_x_bounds.get(),
                *graph_y_bounds.get(),
                false,
            ) {
                starting_position.set(get_pointer_position());
                position.set(get_pointer_position());
                selecting.set(true);
            }
        }
    });

    create_effect(cx, move || {
        if *selecting.get() {
            spawn_local_scoped(cx, async move {
                while *selecting.get() {
                    let new_position = get_pointer_position();
                    if !is_inside_graph(
                        new_position,
                        *graph_x_bounds.get(),
                        *graph_y_bounds.get(),
                        true,
                    ) {
                        selecting.set(false);
                        break;
                    }

                    position.set(new_position);
                    wait_for_pointer_move().await;
                }
            })
        }
    });

    let zoom_rect = create_memo(cx, move || {
        if !*selecting.get() {
            return view! { cx, "" };
        }

        let ((start_x, start_y), (end_x, end_y)) = fix_positions(
            *starting_position.get(),
            *position.get(),
            *props.svg_size.get(),
        );

        let height = (end_y - start_y).to_string();
        let width = (end_x - start_x).to_string();
        let start_x = start_x.to_string();
        let start_y = start_y.to_string();
        view! { cx,
            rect(
                x=start_x,
                y=start_y,
                height=height,
                width=width,
                stroke="#938056",
                rx=0,
                fill="#938056",
                fill-opacity=0.2
            ) {}
        }
    });

    // Undo zoom
    spawn_local_scoped(cx, async move {
        loop {
            wait_for_right_button_down().await;

            let position = get_pointer_position();
            if is_inside_graph(
                position,
                *graph_x_bounds.get(),
                *graph_y_bounds.get(),
                false,
            ) {
                change_limits(None, None).await;
                props.limits_change_flag.set(true);
            }
        }
    });

    view! { cx,
        div(class="graph-space back", id="graph_space") {
            svg(
                width=props.svg_size.get().0,
                height=props.svg_size.get().1)
            {
                GraphFrame(
                    svg_size=props.svg_size,
                    draw_time_series=props.draw_time_series
                )

                clipPath(id="graph-clip") {
                    rect(
                        width=(props.svg_size.get().0 - 44),
                        height=(props.svg_size.get().1 - 20),
                        x="2", y="2") {}
                }

                TimeSeries(
                    series_paths=props.time_series_paths,
                    draw_series=props.draw_time_series,
                )

                (draw_shadow(
                    cx,
                    &*props.shadow_paths.get(),
                    *props.draw_shadow.get()
                ))

                Indexed(
                    iterable=props.traces,
                    view = |cx, trace| {
                        draw_trace(cx, trace)
                    }
                )

                Indexed(
                    iterable=props.traces,
                    view = |cx, trace| {
                        draw_markers(cx, trace)
                    }
                )
                (*zoom_rect.get())
            }
        }
    }
}

#[derive(Prop)]
struct TimeSeriesProps<'a> {
    series_paths: &'a ReadSignal<TimeSeriesGroupPaths>,
    draw_series: &'a ReadSignal<bool>,
}

#[component]
fn TimeSeries<'a, G: Html>(cx: Scope<'a>, props: TimeSeriesProps<'a>) -> View<G> {
    // MARK TODO make the other series: peaks and means. Maybe make a group of
    // Checkboxes to determine what is shown at the time series and put it in
    // the config
    // Also choose some suitable colors for the peaks
    let valleys = create_memo(cx, || {
        let series = props.series_paths.get().valleys.clone();
        if series.is_empty() {
            vec!["".to_string()]
        } else {
            series
        }
    });
    let valley_means = create_memo(cx, || {
        let series = props.series_paths.get().valley_means.clone();
        if series.is_empty() {
            vec!["".to_string()]
        } else {
            series
        }
    });
    let peaks = create_memo(cx, || {
        let series = props.series_paths.get().peaks.clone();
        if series.is_empty() {
            vec!["".to_string()]
        } else {
            series
        }
    });
    let peak_means = create_memo(cx, || {
        let series = props.series_paths.get().peak_means.clone();
        if series.is_empty() {
            vec!["".to_string()]
        } else {
            series
        }
    });

    view! { cx,
        (if *props.draw_series.get() {
            view! { cx,
                Indexed(
                    iterable=valleys,
                    view = |cx, path| {
                        view! { cx, path(
                            d=path,
                            fill="none",
                            stroke-width="1",
                            stroke="#A3D4D5",
                            clip-path="url(#graph-clip)",
                            ) {}
                        }
                    }
                )

                Indexed(
                    iterable=valley_means,
                    view = |cx, path| {
                        view! { cx, path(
                            d=path,
                            fill="none",
                            stroke-width="3",
                            stroke="#A3D4D5",
                            clip-path="url(#graph-clip)",
                            ) {}
                        }
                    }
                )

                Indexed(
                    iterable=peaks,
                    view = |cx, path| {
                        view! { cx, path(
                            d=path,
                            fill="none",
                            stroke-width="1",
                            stroke="#03D4D5",
                            clip-path="url(#graph-clip)",
                            ) {}
                        }
                    }
                )

                Indexed(
                    iterable=peak_means,
                    view = |cx, path| {
                        view! { cx, path(
                            d=path,
                            fill="none",
                            stroke-width="3",
                            stroke="#03D4D5",
                            clip-path="url(#graph-clip)",
                            ) {}
                        }
                    }
                )
            }
        } else {
            view! { cx, "" }
        })
    }
}

fn draw_trace<G: Html>(cx: Scope, trace: Trace) -> View<G> {
    let trace_line = trace.render_spectrum(cx);

    view! { cx,
        (trace_line)
    }
}

fn draw_markers<G: Html>(cx: Scope, trace: Trace) -> View<G> {
    let valleys_markers = trace.render_valleys_markers(cx);
    let peaks_markers = trace.render_peaks_markers(cx);

    let valleys_mean_marker = trace.render_valleys_mean_marker(cx);
    let peaks_mean_marker = trace.render_peaks_mean_marker(cx);

    view! { cx,
        (valleys_markers)
        (peaks_markers)

        (valleys_mean_marker)
        (peaks_mean_marker)
    }
}

#[derive(Prop)]
struct FrameProps<'a> {
    svg_size: &'a ReadSignal<(i32, i32)>,
    draw_time_series: &'a ReadSignal<bool>,
}

#[component]
fn GraphFrame<'a, G: Html>(cx: Scope<'a>, props: FrameProps<'a>) -> View<G> {
    let graph_size = create_memo(cx, || svg_to_graph_size(*props.svg_size.get()));

    let path_sqr = create_memo(cx, || {
        format!(
            "M 1,1 L {},1 L {},{} L 1,{} L 1,1",
            (graph_size.get()).0 - 1, // - 1 pra margem por conta da largura do traço
            (graph_size.get()).0 - 1,
            (graph_size.get()).1 - 1,
            (graph_size.get()).1 - 1
        )
    });

    let n_divs = create_memo(cx, || {
        (
            (graph_size.get()).0 / 100 + 1,
            (graph_size.get()).1 / 62 + 1,
        )
    });

    let divs_x = create_memo(cx, || {
        (1..(n_divs.get()).0)
            .map(|x| (x * (graph_size.get()).0) / (n_divs.get()).0)
            .collect::<Vec<i32>>()
    });

    let divs_x_path = create_memo(cx, || {
        (*divs_x.get())
            .iter()
            .map(|x| format!("M {},1 L {},{}", x, x, (graph_size.get()).1 - 1))
            .collect::<Vec<String>>()
    });

    let divs_y = create_memo(cx, || {
        (1..(n_divs.get()).1)
            .map(|y| (y * (graph_size.get()).1) / (n_divs.get()).1)
            .collect::<Vec<i32>>()
    });

    let divs_y_path = create_memo(cx, || {
        (*divs_y.get())
            .iter()
            .map(|y| format!("M 1,{} L {},{}", y, (graph_size.get()).0 - 1, y))
            .collect::<Vec<String>>()
    });

    let total_time = 5 * 60;
    let time_div_label = create_memo(cx, move || {
        let scale = total_time / (n_divs.get()).1;
        format!("󰹹 {} s/div", scale)
    });

    view! { cx,
        rect(
            width=(graph_size.get().0 - 2),
            height=(graph_size.get().1 - 2),
            fill="#16161D",
            x="1",
            y="1"
        ) {}

        Indexed(
            iterable=divs_x_path,
            view = |cx, x| view! { cx,
                path(
                    d=x,
                    fill="none",
                    stroke-width="1",
                    stroke="#938056",
                    opacity="0.5"
                ) {}
            }
        )

        Indexed(
            iterable=divs_y_path,
            view = |cx, x| view! { cx,
                path(
                    d=x,
                    fill="none",
                    stroke-width="1",
                    stroke="#938056",
                    opacity="0.5"
                ) {}
            }
        )

        GraphLabels(graph_size=graph_size, divs_x=divs_x, divs_y=divs_y)

        path(
            d=path_sqr.get(),
            fill="none",
            stroke-width="2",
            stroke="#938056"
        ) {}

        text(
            x=1,
            y=(graph_size.get().1 + 13),
            font-size="0.75rem",
            fill="#938056"
        ) { "Comp. de Onda (nm)" }

        text(
            x=(graph_size.get().0 + 4),
            y=12,
            font-size="0.75rem",
            fill="#938056"
        ) { "Pot." }

        text(
            x=(graph_size.get().0 + 4),
            y=24,
            font-size="0.75rem",
            fill="#938056"
        ) { "(dB)" }

        (if *props.draw_time_series.get() {
            view! { cx,
                text(
                    x=5,
                    y=15,
                    font-size="0.75rem",
                    fill="#938056"
                ) { "Série Temp." }

                text(
                    x=5,
                    y=30,
                    font-size="0.75rem",
                    fill="#938056"
                ) { (*time_div_label.get()) }
                }
        } else {
            view! { cx, "" }
        })
    }
}

#[derive(Prop)]
struct LabelsProps<'a> {
    graph_size: &'a ReadSignal<(i32, i32)>,
    divs_x: &'a ReadSignal<Vec<i32>>,
    divs_y: &'a ReadSignal<Vec<i32>>,
}

#[component]
fn GraphLabels<'a, G: Html>(cx: Scope<'a>, props: LabelsProps<'a>) -> View<G> {
    let wl_limits = create_signal(cx, (1500f64, 1600f64));
    spawn_local_scoped(cx, async move {
        // Updates wl limits
        loop {
            TimeoutFuture::new(200).await; // 5 fps, #TODO send to config
            let mut new_wl_limits = get_wavelength_limits().await;
            if new_wl_limits.0 < 1.0 {
                // If lower, it is in meters (~1e-6)
                new_wl_limits = (new_wl_limits.0 * 1e9, new_wl_limits.1 * 1e9); // To nanometers
            }
            if new_wl_limits != *wl_limits.get() {
                wl_limits.set(new_wl_limits);
            }
        }
    });

    let wl_limits_txt = create_memo(cx, || {
        (*props.divs_x.get())
            .iter()
            .skip(1)
            .map(|x| {
                (
                    x,
                    (wl_limits.get()).0
                        + ((wl_limits.get()).1 - (wl_limits.get()).0) * (*x as f64)
                            / (props.graph_size.get()).0 as f64,
                )
            })
            .map(|(pos, x)| (*pos, format!("{:.2}", x)))
            .collect::<Vec<(i32, String)>>()
    });

    let pwr_limits = create_signal(cx, (3f64, -50f64));
    spawn_local_scoped(cx, async move {
        // Updates power limits
        loop {
            TimeoutFuture::new(200).await; // 30 fps, #TODO send to config
            let new_pwr_limits = get_power_limits().await;
            if new_pwr_limits != *pwr_limits.get() {
                pwr_limits.set(new_pwr_limits);
            }
        }
    });

    let pwr_limits_txt = create_memo(cx, || {
        (*props.divs_y.get())
            .iter()
            .map(|y| {
                (
                    y,
                    (pwr_limits.get()).0
                        + ((pwr_limits.get()).1 - (pwr_limits.get()).0) * (*y as f64)
                            / (props.graph_size.get()).1 as f64,
                )
            })
            .map(|(pos, y)| (*pos + 4, format!("{:.1}", y)))
            .collect::<Vec<(i32, String)>>()
    });

    view! { cx,
        Indexed(
            iterable=wl_limits_txt,
            view = move |cx, (pos, txt)| view! { cx,
                text(
                    x=pos,
                    y=(props.graph_size.get().1 + 13),
                    font-size="0.75rem",
                    text-anchor="middle",
                    fill="#c0a36e"
                ) { (txt) }
            }
        )
        Indexed(
            iterable=pwr_limits_txt,
            view = move |cx, (pos, txt)| view! { cx,
                text(
                    x=(props.graph_size.get().0 + 4),
                    y=pos,
                    font-size="0.75rem",
                    fill="#c0a36e"
                ) { (txt) }
            }
        )
    }
}

fn draw_shadow<G: Html>(cx: Scope, shadow_paths: &[String], draw_shadow: bool) -> View<G> {
    let shadow_paths = shadow_paths.to_owned();

    let rendered = if draw_shadow {
        View::new_fragment(
            shadow_paths
                .into_iter()
                .rev()
                .enumerate()
                .map(|(i, path)| {
                    view! { cx,
                        path(
                            d=path,
                            fill="none",
                            stroke-width="3",
                            stroke="#C34043",
                            opacity=(0.3 + 0.8_f64.powi(i as i32)),
                            clip-path="url(#graph-clip)",
                        ) {}
                    }
                })
                .collect(),
        )
    } else {
        view! { cx, "" }
    };

    view! { cx,
        // NOTE this filter is way hevier than what I expected (on my linux laptop, at least)
        // If this becomes a problem, I'll need to think of something else, or maybe
        // rendering to canvas instead of svg, in order to use hardware acceleration
        filter(id="scatter") {
            feGaussianBlur(in="SourceGraphic", stdDeviation="3")
        }
        g(filter="url(#scatter)") {
            (rendered)
        }
    }
}
