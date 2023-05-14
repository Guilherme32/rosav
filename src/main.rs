use sycamore::prelude::*;
// use itertools::Itertools;
// use std::iter;

use sycamore::futures::{ spawn_local_scoped };

use gloo_timers::future::TimeoutFuture;

pub mod api;
use api::*;


fn main() {
    sycamore::render(|cx| view!{ cx,
        div(class="horizontal-container") {
            SideBar {}
            div(class="vertical-container") {
                Graph {}
                LowerBar {}
            }
        }
    })
}


// COMPONENTS ----------------------------

#[component]
fn Graph<G:Html>(cx: Scope) -> View<G> {
    let is_ready = create_signal(cx, false);
    let path = create_signal(cx, String::new());
    let svg_size = create_signal(cx, (0i32, 0i32));

    spawn_local_scoped(cx, async move {
        loop {
            TimeoutFuture::new(200).await;
            if unread_spectrum().await {
                if let Some(spectrum_path) = get_last_spectrum_path().await {
                    path.set(spectrum_path);
                }
            }
            is_ready.set(unread_spectrum().await);

            svg_size.set(get_svg_size().await);
        }
    });

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
                            width=svg_size.get().0,
                            height=svg_size.get().1)
                        {
                            GraphFrame(svg_size=svg_size)
                            path(d=path.get(), fill="none", stroke="#000000", stroke-width="3") {}
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
        (*graph_size.get()).1 / 100 + 1) 
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
                path(d=x, fill="none", stroke-width="1", stroke="gray") {}
            }
        )
        Indexed(
            iterable=divs_y_path,
            view = |cx, x| view! { cx,
                path(d=x, fill="none", stroke-width="1", stroke="gray") {}
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
    spawn_local_scoped(cx, async move {
        loop {
            TimeoutFuture::new(200).await;
            let new_wl_limits = get_wavelength_limits().await;
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
    spawn_local_scoped(cx, async move {
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

#[component]
fn SideBar<G:Html>(cx: Scope) -> View<G> {
    view! { cx,
        div(class="side-bar") {
            SideBarMain {}
            LogSpace {}
        }
    }
}

#[derive(Clone, PartialEq, Debug)]
struct Trace {
    id: u8,
    visible: bool,
    draw_valleys: bool,                // TODO adicionar detecção de vale
    active: bool,
    valleys: Vec<f64>,
    svg_size: (i32, i32),
    svg_path: String,
    freeze_time: Option<String>        // Se None não está congelado
}

fn new_trace(id: u8) -> Trace {
    Trace {
        id,
        visible: true,
        draw_valleys: true,
        active: true,
        valleys: vec![],
        svg_size: (0, 0),
        svg_path: String::new(),
        freeze_time: None
    }
}

fn trace_id_to_name(id: u8) -> String {
    if id > 25 {
        format!("{}", id)
    } else {
        let letters = vec!["A", "B", "C", "D", "E", "F", "G", "H", "I", "J",
                           "K", "L", "M", "N", "O", "P", "Q", "R", "S", "T",
                           "U", "V", "W", "X", "Y", "Z"];
        format!("{}", letters[id as usize])
    }
}

#[derive(Prop)]
struct RenderTraceProps<'a> {
    trace: Trace,
    traces_list: &'a Signal<Vec<Trace>>
}

async fn freeze_callback<'a>(id: u8, traces_list: &'a Signal<Vec<Trace>>) {
    for mut trace in traces_list.modify().iter_mut() {
        if trace.id != id {
            continue;
        }

        trace.freeze_time = Some(get_time().await);
        trace.active = false;
        // TODO mandar congelar no backend tb

        break;
    }
}

#[component]
fn RenderTrace<'a, G:Html>(cx: Scope<'a>, props: RenderTraceProps<'a>) -> View<G> {
    let freeze = move |_| {
        spawn_local_scoped(cx, async move {
            freeze_callback(props.trace.id, props.traces_list).await;
        })
    };

    view! { cx, 
        div(class="trace") {
            span(class="name") { (trace_id_to_name(props.trace.id)) }
            span(class="status") {
                (match &props.trace.freeze_time {
                    Some(time) => time.clone(),
                    None => "(Ativo)".to_string()
                })
            }
            div(class="buttons") {
                (match props.trace.active {
                    true => view! { cx, button(on:click=freeze) { " " } },
                    false => view! { cx, button() { "󰜺 " } }
                })
                (if props.trace.visible {
                    view! { cx, button() { " " } }
                } else {
                    view! { cx, button() { " " } }
                })
                button() { " " }
                (if props.trace.draw_valleys {
                    view! { cx, button() { "󰽅 " } }
                } else {
                    view! { cx, button() { "󰆣 " } }
                })
            }
        }
    }
}

#[component]
fn SideBarMain<G:Html>(cx: Scope) -> View<G> {
    let traces = create_signal(cx, vec![
        new_trace(0),
        new_trace(1),
        new_trace(2)
    ]);

    view! { cx,
        div(class="side-bar-main") {
            p(class="title") { "Traços" }

            div(class="trace-container back") {
                Indexed(
                    iterable = traces,
                    view = move |cx, trace| view! { cx, RenderTrace(trace=trace, traces_list=&traces) }
                )
                div(class="trace") {
                   span(class="name") { "E" }
                    span(class="status") { "(Ativo)" }
                    div(class="buttons") {
                        button() { "󰜺 " }
                        button() { " " }
                        button() { " " }
                        button() { "⚡" }
                    }
                }
            }

            div(class="trace-paging") {
                button() { " << " }
                p() { "10/10" }
                button() { " >> " }
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