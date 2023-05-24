use sycamore::prelude::*;
use sycamore::futures::spawn_local_scoped;
use gloo_timers::future::TimeoutFuture;

use std::path::PathBuf;

use crate::api::*;
use crate::trace::*;
use crate::ActiveSide;


#[derive(Prop)]
pub struct SideBarProps<'a> {
    traces: &'a Signal<Vec<Trace>>,
    saving: &'a Signal<bool>,
    active_side: &'a ReadSignal<ActiveSide>
}

#[component]
pub fn SideBar<'a, G:Html>(cx: Scope<'a>, props: SideBarProps<'a>) -> View<G> {
    view! { cx,
        div(class="side-bar") {
            (match *props.active_side.get() {
                ActiveSide::Traces => 
                    view! { cx, 
                        SideBarMain(
                            traces=props.traces,
                            saving=props.saving
                        )
                    },
                ActiveSide::Config =>
                    view! { cx,
                        ConfigWindow {}
                    }
            })
            LogSpace {}
        }
    }
}

#[derive(Prop)]
struct RenderTraceProps<'a> {
    trace: Trace,
    traces_list: &'a Signal<Vec<Trace>>,
    saving: &'a Signal<bool>
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

async fn save_frozen_callback<'a>(id: u8, _traces_list: &'a Signal<Vec<Trace>>) {
    save_frozen_spectrum(id as usize).await;
}

async fn save_continuous_callback<'a>(saving: &'a Signal<bool>) {
    save_continuous(!*saving.get()).await;
    saving.set(get_saving().await);
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
    let save_frozen = move |_| {
        spawn_local_scoped(cx, async move {
            save_frozen_callback(props.trace.id, props.traces_list).await;
        })
    };
    let save_continuous = move |_| {
        spawn_local_scoped(cx, async move {
            save_continuous_callback(props.saving).await;
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

                (if props.trace.active {
                    if *props.saving.get() {
                        view! { cx, button(on:click=save_continuous) { "󱧹 " } }
                    } else {
                        view! { cx, button(on:click=save_continuous) { "󱃩 " } }
                    }
                } else {
                    view! { cx, button(on:click=save_frozen) { " " } }
                })

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
    traces: &'a Signal<Vec<Trace>>,
    saving: &'a Signal<bool>
}

#[component]
fn SideBarMain<'a, G:Html>(cx: Scope<'a>, props: SideBarMainProps<'a>) -> View<G> {
    view! { cx,
        div(class="side-bar-main") {
            p(class="title") { "Traços" }

            div(class="side-container back") {
                Indexed(
                    iterable = props.traces,
                    view = move |cx, trace| view! { 
                        cx, RenderTrace(
                            trace=trace,
                            traces_list=&props.traces,
                            saving=&props.saving
                        )
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
        // let mut count = 0u32;
        loop {
            TimeoutFuture::new(200).await;
            let new_logs = get_last_logs().await;
            for new_log in new_logs {
                // new_log.id = count;
                logs.modify().push(new_log);
                // count += 1;
            }
        }
    });

    view! { cx,
        div(class="side-bar-log") {
            div(class="title") { "Registro" }
            div(class="log-space back") {
                Indexed(
                    iterable = logs,
                    view = |cx, x| view! { cx,
                        p(class=x.log_type) { (x.msg) }
                    }
                    // key = |x| (*x).id,
                )
            }
        }
    }
}

#[component]
fn ConfigWindow<G:Html>(cx: Scope) -> View<G> {
    let file = create_signal(cx, PathBuf::new());
    let file_display = create_memo(cx, || {
        match (*file.get()).to_str() {
            None => "".to_string(),
            Some(txt) => txt.clone().to_string()
        }
    });

    let find_path = move |_| {
        spawn_local_scoped(cx, async move {
            match get_path().await {
                None => (),
                Some(path) => file.set(path)
            }
        });
    };

    view! { cx, 
        div(class="side-bar-main") {
            p(class="title") { "Configurações" }
            div(class="side-container back") {
                p { "Configs" }
                
                button(on:click=find_path) { "meclica" }
                
                p { (file_display.get()) }
            }
        }
    }
}
