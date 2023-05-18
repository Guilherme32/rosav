use sycamore::prelude::*;
// use itertools::Itertools;
// use std::iter;

use sycamore::futures::{ spawn_local_scoped };

use gloo_timers::future::TimeoutFuture;

pub mod api;
use api::*;

pub mod trace;
use trace::*;


fn main() {
    sycamore::render(|cx| view!{ cx,
        Main {}
    })
}


// COMPONENTS ----------------------------

#[component]
fn Main<G:Html>(cx: Scope) -> View<G> {
    let traces = create_signal(cx, vec![new_trace(0)]);

    let svg_size = create_signal(cx, (0i32, 0i32));

    spawn_local_scoped(cx, async move {
        loop {
            TimeoutFuture::new(200).await;
            if unread_spectrum().await {                // Get the latest spectrum if it is available
                let new_path = get_last_spectrum_path().await;
                traces.modify().last_mut().map(|trace| {
                    trace.svg_size = *svg_size.get();
                    trace.svg_path = new_path;
                });
                continue;                // Skip the loop to end the modify() and avoid problems
            }

            let new_svg_size = get_svg_size().await;
            for (id, trace) in traces.modify().iter_mut().enumerate() {    // Update when the window changes
                if trace.svg_path.len() == 0 {                             // No old spectrum, no update
                    continue;
                }
                if trace.svg_size !=  new_svg_size {
                    trace.svg_size = new_svg_size;
                    if trace.active {
                        trace.svg_path = get_last_spectrum_path().await;
                    } else {
                        trace.svg_path = get_frozen_spectrum_path(id).await;
                    }
                }
            }
            svg_size.set(new_svg_size);
        }
    });

    view!{ cx,
        div(class="horizontal-container") {
            SideBar(traces=traces)
            div(class="vertical-container") {
                Graph(traces=traces, svg_size=svg_size)
                LowerBar {}
            }
        }
    }
}

#[derive(Prop)]
struct GraphProps<'a> {
    svg_size: &'a ReadSignal<(i32, i32)>,
    traces: &'a ReadSignal<Vec<Trace>>
}

#[component]
fn Graph<'a, G:Html>(cx: Scope<'a>, props: GraphProps<'a>) -> View<G> {
    // let is_ready = create_signal(cx, false);
    // let path = create_signal(cx, String::new());
    // let svg_size = create_signal(cx, (0i32, 0i32));

    // spawn_local_scoped(cx, async move {
    //     loop {
    //         TimeoutFuture::new(200).await;
    //         if unread_spectrum().await {
    //             if let Some(spectrum_path) = get_last_spectrum_path().await {
    //                 path.set(spectrum_path);
    //             }
    //         }
    //         is_ready.set(unread_spectrum().await);

    //         svg_size.set(get_svg_size().await);
    //     }
    // });

    view! { cx,
        div(class="graph-space back") {
            (if false {//(*path.get()).len() == 0 {
                view! { cx,
                    div(class="placeholder") {
                        p { "Área do gráfico" }
                        p { "Sem espectro para mostrar" }
                    }
                }
            } else {
                    view! { cx,
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
                                view = |cx, trace| if trace.visible { 
                                    view! { cx,
                                        path(
                                            d=trace.svg_path,
                                            fill="none",
                                            stroke-width="2",
                                            stroke=trace_id_to_color(trace.id),
                                            clip-path="url(#graph-clip)"
                                            ) {}
                                    } 
                                } else { view! { cx, "" } }
                            )
                        }
                    }
                }
            )
        }
    }
}

#[derive(Prop)]
struct FrameProps<'a> {
    svg_size: &'a ReadSignal<(i32, i32)>
}

#[component]
fn GraphFrame<'a, G:Html>(cx: Scope<'a>, props: FrameProps<'a>) -> View<G> {

    let graph_size = create_memo(cx, || 
        ((*props.svg_size.get()).0 - 40,        // 32 e 16 para os labels dos eixos
        (*props.svg_size.get()).1 - 16)
        
    );

    let path_sqr = create_memo(cx, || 
        format!("M 1,1 L {},1 L {},{} L 1,{} L 1,1",
            (*graph_size.get()).0 - 1,        // - 1 pra margem por conta ta largura do traço
            (*graph_size.get()).0 - 1, (*graph_size.get()).1 - 1,
            (*graph_size.get()).1 - 1
        )
    );

    let n_divs = create_memo(cx, || 
       ((*graph_size.get()).0 / 100 + 1,
        (*graph_size.get()).1 / 50 + 1) 
    );

    let divs_x = create_memo(cx, ||
        (1..(*n_divs.get()).0)
            .map(|x| (x * (*graph_size.get()).0) / (*n_divs.get()).0)
            .collect::<Vec<i32>>()
    );

    let divs_x_path = create_memo(cx, || 
        (*divs_x.get()).iter()
            .map(|x| format!("M {},1 L {},{}", x, x, (*graph_size.get()).1 - 1))
            .collect::<Vec<String>>()
    );

    let divs_y = create_memo(cx, ||
        (1..(*n_divs.get()).1)
            .map(|y| (y * (*graph_size.get()).1) / (*n_divs.get()).1)
            .collect::<Vec<i32>>()
    );

    let divs_y_path = create_memo(cx, || 
        (*divs_y.get()).iter()
            .map(|y| format!("M 1,{} L {},{}", y, (*graph_size.get()).0 - 1, y))
            .collect::<Vec<String>>()
    );


    view! { cx,
        rect(width=(graph_size.get().0 - 2), height=(graph_size.get().1 - 2), 
            fill="white", x="1", y="1") {}
        Indexed(
            iterable=divs_x_path,
            view = |cx, x| view! { cx,
                path(d=x, fill="none", stroke-width="1", stroke="lightgray") {}
            }
        )
        Indexed(
            iterable=divs_y_path,
            view = |cx, x| view! { cx,
                path(d=x, fill="none", stroke-width="1", stroke="lightgray") {}
            }
        )

        GraphLabels(graph_size=graph_size, divs_x=divs_x, divs_y=divs_y)

        path(d=path_sqr.get(), fill="none",
            stroke-width="2", stroke="#000000") {}
        text(x=1, y=(graph_size.get().1 + 13), font-size="0.75rem") {
            "Comp. de Onda (nm)"
        }
        text(x=(graph_size.get().0 + 4), y=12, font-size="0.75rem") {
            "Pot."
        }
        text(x=(graph_size.get().0 + 4), y=24, font-size="0.75rem") {
            "(dB)"
        }
    }

}

#[derive(Prop)]
struct LabelsProps<'a> {
    graph_size: &'a ReadSignal<(i32, i32)>,
    divs_x: &'a ReadSignal<Vec<i32>>,
    divs_y: &'a ReadSignal<Vec<i32>>
}

#[component]
fn GraphLabels<'a, G:Html>(cx: Scope<'a>, props: LabelsProps<'a>) -> View<G> {

    let wl_limits = create_signal(cx, (1500f64, 1600f64));
    spawn_local_scoped(cx, async move {                                // Updates wl limits
        loop {
            TimeoutFuture::new(200).await;
            let mut new_wl_limits = get_wavelength_limits().await;
            if new_wl_limits.0 < 1.0 {                             // If lower, it is in meters (~1e-6)
                new_wl_limits = (new_wl_limits.0*1e9, new_wl_limits.1*1e9);        // To nanometers
            }
            if new_wl_limits != *wl_limits.get() {
                wl_limits.set(new_wl_limits);
            }
        }
    });

    let wl_limits_txt = create_memo(cx, ||
        (*props.divs_x.get()).iter()
            .skip(1)
            .map(|x|
                (x,
                (*wl_limits.get()).0
                + ((*wl_limits.get()).1 - (*wl_limits.get()).0)
                * (*x as f64) / (*props.graph_size.get()).0 as f64)
            ).map(|(pos, x)| (*pos, format!("{:.2}", x)))
            .collect::<Vec<(i32, String)>>()
    );

    let pwr_limits = create_signal(cx, (3f64, -50f64));
    spawn_local_scoped(cx, async move {                        // Updates power limits
        loop {
            TimeoutFuture::new(200).await;
            let new_pwr_limits = get_power_limits().await;
            if new_pwr_limits != *pwr_limits.get() {
                pwr_limits.set(new_pwr_limits);
            }
        }
    });

    let pwr_limits_txt = create_memo(cx, ||
        (*props.divs_y.get()).iter()
            .map(|y|
                (y,
                (*pwr_limits.get()).0
                + ((*pwr_limits.get()).1 - (*pwr_limits.get()).0)
                * (*y as f64) / (*props.graph_size.get()).1 as f64)
            ).map(|(pos, y)| (*pos + 4, format!("{:.1}", y)))
            .collect::<Vec<(i32, String)>>()
    );

    view! { cx,
        Indexed(
            iterable=wl_limits_txt,
            view = move |cx, (pos, txt)| view! { cx,
                text(x=pos, y=(props.graph_size.get().1 + 13), font-size="0.75rem",
                     text-anchor="middle") {
                    (txt)
                }
            }
        )
        Indexed(
            iterable=pwr_limits_txt,
            view = move |cx, (pos, txt)| view! { cx,
                text(x=(props.graph_size.get().0 + 4), y=pos, font-size="0.75rem") {
                    (txt)
                }
            }
        )
    }
}

#[component]
fn LowerBar<G:Html>(cx: Scope) -> View<G> {
    view! { cx, 
        div(class="lower-bar back") {
            div() {
                button() { "󰢻 "}
                button() { "󰽉 "}
            }
            div() {
                button(class="no-offset") { " " }
                button(style="padding-right: 0.6rem;") { "󱑹 " }
                button(class="no-offset") { "󱐥 " }
                button() { "󱐤 " }
            }
            div(class="status") {
                p() { "Lendo Const." }
                p() { "Não Salvando" }
            }
        }
    }
}

#[derive(Prop)]
struct SideBarProps<'a> {
    traces: &'a Signal<Vec<Trace>>
}

#[component]
fn SideBar<'a, G:Html>(cx: Scope<'a>, props: SideBarProps<'a>) -> View<G> {
    view! { cx,
        div(class="side-bar") {
            SideBarMain(traces=props.traces)
            LogSpace {}
        }
    }
}

#[derive(Prop)]
struct RenderTraceProps<'a> {
    trace: Trace,
    traces_list: &'a Signal<Vec<Trace>>
}

async fn freeze_callback<'a>(id: u8, traces_list: &'a Signal<Vec<Trace>>) {
    let mut traces_list = traces_list.modify();

    let trace = &mut traces_list[id as usize];
    if trace.svg_path.len() == 0 {        // Nao pode congelar onde não tem espectro
        return ();
    }

    trace.freeze_time = Some(get_time().await);
    trace.active = false;

    traces_list.push(new_trace(id+1));

    freeze_spectrum().await;
}

async fn delete_callback<'a>(id: u8, traces_list: &'a Signal<Vec<Trace>>) {
    traces_list.modify().remove(id as usize);

    for (i, mut trace) in traces_list.modify().iter_mut().enumerate() {
        trace.id = i as u8;
    }

    delete_frozen_spectrum(id as usize).await;
}

async fn visibility_callback<'a>(id: u8, traces_list: &'a Signal<Vec<Trace>>) {
    let trace = &mut traces_list.modify()[id as usize];
    trace.visible = !trace.visible;
}

async fn save_callback<'a>(_id: u8, _traces_list: &'a Signal<Vec<Trace>>) {
    print_backend(&format!("{:?}", _traces_list)).await;
    hello().await;
    // TODO mandar salvar o espectro pelo backend
}

async fn draw_valleys_callback<'a>(id: u8, traces_list: &'a Signal<Vec<Trace>>) {
    let trace = &mut traces_list.modify()[id as usize];
    trace.draw_valleys = !trace.draw_valleys;
}

#[component]
fn RenderTrace<'a, G:Html>(cx: Scope<'a>, props: RenderTraceProps<'a>) -> View<G> {
    let freeze = move |_| {
        spawn_local_scoped(cx, async move {
            freeze_callback(props.trace.id, props.traces_list).await;
        })
    };
    let delete = move |_| {
        spawn_local_scoped(cx, async move {
            delete_callback(props.trace.id, props.traces_list).await;
        })
    };
    let visibility = move |_| {
        spawn_local_scoped(cx, async move {
            visibility_callback(props.trace.id, props.traces_list).await;
        })
    };
    let save = move |_| {
        spawn_local_scoped(cx, async move {
            save_callback(props.trace.id, props.traces_list).await;
        })
    };
    let draw_valleys = move |_| {
        spawn_local_scoped(cx, async move {
            draw_valleys_callback(props.trace.id, props.traces_list).await;
        })
    };

    view! { cx, 
        div(class="trace") {
            span(class="name", style=trace_id_to_style(props.trace.id)) {
                (trace_id_to_name(props.trace.id))
            }
            span(class="status") {
                (match &props.trace.freeze_time {
                    Some(time) => time.clone(),
                    None => "(Ativo)".to_string()
                })
            }
            div(class="buttons") {
                (match props.trace.active {
                    true => view! { cx, button(on:click=freeze) { " " } },
                    false => view! { cx, button(on:click=delete) { "󰜺 " } }
                })
                (if props.trace.visible {
                    view! { cx, button(on:click=visibility) { " " } }
                } else {
                    view! { cx, button(on:click=visibility) { " " } }
                })
                button(on:click=save) { " " }
                (if props.trace.draw_valleys {
                    view! { cx, button(on:click=draw_valleys) { "󰽅 " } }
                } else {
                    view! { cx, button(on:click=draw_valleys) { "󰆤 " } }
                })
            }
        }
    }
}

#[derive(Prop)]
struct SideBarMainProps<'a> {
    traces: &'a Signal<Vec<Trace>>
}

#[component]
fn SideBarMain<'a, G:Html>(cx: Scope<'a>, props: SideBarMainProps<'a>) -> View<G> {
    // let traces = create_signal(cx, vec![new_trace(0)]);

    // create_effect(cx, move || {
    //     let msg = format!("\nTraces changed: {:?}\n", traces);
    //     spawn_local_scoped(cx, async move {
    //         print_backend(&msg).await;
    //     })
    // });

    view! { cx,
        div(class="side-bar-main") {
            p(class="title") { "Traços" }

            div(class="trace-container back") {
                Indexed(
                    iterable = props.traces,
                    view = move |cx, trace| view! { 
                        cx, RenderTrace(trace=trace, traces_list=&props.traces)
                    }
                )
            }
        }
    }
}

#[component]
fn LogSpace<G:Html>(cx: Scope) -> View<G> {
    let logs = create_signal(cx, Vec::<Log>::with_capacity(30));

    spawn_local_scoped(cx, async move {
        let mut count = 0u32;
        loop {
            TimeoutFuture::new(200).await;
            let new_logs = get_last_logs().await;
            for mut new_log in new_logs {
                new_log.id = count;
                logs.modify().push(new_log);
                count += 1;
            }
        }
    });

    view! { cx,
        div(class="side-bar-log") {
            div(class="title") { "Registro" }
            div(class="log-space back") {
                Keyed(
                    iterable = logs,
                    view = |cx, x| view! { cx,
                        p(class=x.log_type) { (x.msg) }
                    },
                    key = |x| (*x).id,
                )
            }
        }
    }
}