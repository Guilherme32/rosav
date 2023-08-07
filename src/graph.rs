use gloo_timers::future::TimeoutFuture;
use sycamore::futures::spawn_local_scoped;
use sycamore::prelude::*;

use crate::api::*;
use crate::trace::*;

#[derive(Prop)]
pub struct GraphProps<'a> {
    svg_size: &'a ReadSignal<(i32, i32)>,
    traces: &'a ReadSignal<Vec<Trace>>,
}

#[component]
pub fn Graph<'a, G: Html>(cx: Scope<'a>, props: GraphProps<'a>) -> View<G> {
    view! { cx,
        div(class="graph-space back") {
            svg(
                width=props.svg_size.get().0,
                height=props.svg_size.get().1)
            {
                GraphFrame(svg_size=props.svg_size)
                clipPath(id="graph-clip") {
                    rect(
                        width=(props.svg_size.get().0 - 44),
                        height=(props.svg_size.get().1 - 20),
                        x="2", y="2") {}
                }
                Indexed(
                    iterable=props.traces,
                    view = |cx, trace| {
                        draw_trace(cx, trace)
                    }
                )
            }
        }
    }
}

fn draw_trace<G: Html>(cx: Scope, trace: Trace) -> View<G> {
    let valleys_markers = trace.render_valleys_markers(cx);
    let valleys_mean_marker = trace.render_valleys_mean_marker(cx);
    let trace_line = trace.render_spectrum(cx);

    view! { cx,
        (trace_line)
        (valleys_markers)
        (valleys_mean_marker)
    }
}

#[derive(Prop)]
struct FrameProps<'a> {
    svg_size: &'a ReadSignal<(i32, i32)>,
}

#[component]
fn GraphFrame<'a, G: Html>(cx: Scope<'a>, props: FrameProps<'a>) -> View<G> {
    let graph_size = create_memo(cx, || {
        (
            (props.svg_size.get()).0 - 40, // 32 e 16 para os labels dos eixos
            (props.svg_size.get()).1 - 16,
        )
    });

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
