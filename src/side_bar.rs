use sycamore::{ rt, prelude::* };
use sycamore::futures::spawn_local_scoped;
use gloo_timers::future::TimeoutFuture;

use crate::api::*;
use acquisitors::*;
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
        loop {
            TimeoutFuture::new(200).await;
            let new_logs = get_last_logs().await;
            for new_log in new_logs {
                logs.modify().push(new_log);
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
                )
            }
        }
    }
}

#[component]
fn ConfigWindow<G:Html>(cx: Scope) -> View<G> {
    let handler_config = create_signal(cx, empty_handler_config());

    view! { cx, 
        div(class="side-bar-main") {
            div(class="side-container back config") {
                p(class="title") { "Configurações" }
                RenderHandlerConfig (config=handler_config)
                RenderAcquisitorConfig (handler_config=handler_config)
            }
        }
    }
}

#[derive(Prop)]
struct HandlerConfigProps<'a> {
    config: &'a Signal<HandlerConfig>
}

#[component]
fn RenderHandlerConfig<'a, G:Html>(cx: Scope<'a>, props: HandlerConfigProps<'a>) -> View<G> {
    let wl_min = create_signal(cx, String::new());
    let wl_max = create_signal(cx, String::new());

    let pwr_min = create_signal(cx, String::new());
    let pwr_max = create_signal(cx, String::new());

    spawn_local_scoped(cx, async move {                // Get old config
        let _config = get_handler_config().await;
        if let Some(wl_limits) = _config.wavelength_limits {        // Update wl limits input
            wl_min.set(format!("{:.1}", wl_limits.0 * 1e9));
            wl_max.set(format!("{:.1}", wl_limits.1 * 1e9));
        }

        if let Some(pwr_limits) = _config.power_limits {            // Update pwr limits input
            pwr_min.set(format!("{}", pwr_limits.0));
            pwr_max.set(format!("{}", pwr_limits.1));
        }

        props.config.set(_config);                                        // Update whole config
    });

    let update_save_path = move |_| {
        spawn_local_scoped(cx, async move {
            match pick_folder().await {
                None => (),
                Some(path) => (*props.config.modify()).auto_save_path = path
            }
        });
    };

    let save_path = create_memo(cx, || {
        format!("{}", (*props.config.get()).auto_save_path.display())
    });

    let update_limits = move |event: rt::Event| {
        event.prevent_default();
        update_wavelength_limits(wl_min, wl_max, props.config);
        update_power_limits(pwr_min, pwr_max, props.config);
    };

    create_effect(cx, move || {                    // Apply config when it is updated
        props.config.track();
        spawn_local_scoped(cx, async move {
            if *props.config.get() != empty_handler_config() {
                apply_handler_config((*props.config.get()).clone()).await;
            }
        });
    });

    view! { cx, 
        form(class="side-container back config", on:submit=update_limits) {
            input(type="submit", style="display: none;")

            p(class="mini-title") { "Backend Geral" }

            div(class="element") {
                p { "Caminho do auto save:" }
                p { 
                    button(on:click=update_save_path) { " " }
                    (save_path.get())
                }
            }

            div(class="element") {
                p { "Limites do comp. de onda:"}
                p {
                    input(
                        bind:value=wl_min,
                        on:input=|_| check_number_input(wl_min),
                        on:focusout=update_limits
                    ) {}
                    input(
                        bind:value=wl_max,
                        on:input=|_| check_number_input(wl_max),
                        on:focusout=update_limits
                    ) {}
                    "(nm)"
                }
            }

            div(class="element") {
                p { "Limites da potência:"}
                p {
                    input(
                        bind:value=pwr_min,
                        on:input=|_| check_number_input(pwr_min),
                        on:focusout=update_limits
                    ) {}
                    input(
                        bind:value=pwr_max,
                        on:input=|_| check_number_input(pwr_max),
                        on:focusout=update_limits
                    ) {}
                    "(dB)"
                }
            }

            div(class="element") {
                p { "Tipo de aquisitor:" }            // TODO implementar mudança quando passar o outro aquisitor
                select(name="acquisitor") {
                    option(value="file_reader") { "Leitor de arquivos" }
                    option(value="other") { "Outro de teste" }
                }
            }
        }
    }
}

fn update_wavelength_limits(
    wl_min: &ReadSignal<String>,
    wl_max: &ReadSignal<String>,
    config: &Signal<HandlerConfig>
) {
    let new_limits: Option<(f64, f64)>;

    if wl_min.get().len() == 0 || wl_max.get().len() == 0 {
        new_limits = None;
    } else {
        let min_float = wl_min.get().parse::<f64>();
        let max_float = wl_max.get().parse::<f64>();

        new_limits = match (min_float, max_float) {
            (Ok(min), Ok(max)) => Some((min * 1e-9, max * 1e-9)),
            (_, _) => None
        };
    }

    if new_limits != config.get().wavelength_limits {
        config.modify().wavelength_limits = new_limits;
    }
}

fn update_power_limits(
    pwr_min: &ReadSignal<String>,
    pwr_max: &ReadSignal<String>,
    config: &Signal<HandlerConfig>
) {
    let new_limits: Option<(f64, f64)>;

    if pwr_min.get().len() == 0 || pwr_max.get().len() == 0 {
        new_limits = None;
    } else {

        let min_float = pwr_min.get().parse::<f64>();
        let max_float = pwr_max.get().parse::<f64>();

        new_limits = match (min_float, max_float) {
            (Ok(min), Ok(max)) => Some((min, max)),
            (_, _) => None
        };
    }

    if new_limits != config.get().power_limits {
        config.modify().power_limits = new_limits;
    }
}

fn check_number_input(input: &Signal<String>) {
    let mut temp_copy = (*input.get()).clone();
    temp_copy.push_str("1");
    if let Err(_) = temp_copy.parse::<f64>() {
        input.set(String::new());
    }
}

#[derive(Prop)]
struct AcquisitorConfigProps<'a> {
    handler_config: &'a Signal<HandlerConfig>
}

#[component]
fn RenderAcquisitorConfig<'a, G:Html>(cx: Scope<'a>, props: AcquisitorConfigProps<'a>) -> View<G> {
    view! { cx, 
        (match props.handler_config.get().acquisitor {
            AcquisitorSimple::FileReader => view! { cx, 
                RenderFileReaderConfig {}
            }
        })
    }
}

#[component]
fn RenderFileReaderConfig<G:Html>(cx: Scope) -> View<G> {
    let config = create_signal(cx, empty_file_reader_config());

    spawn_local_scoped(cx, async move {                // Get old config
        let _config = get_acquisitor_config().await;
        if let AcquisitorConfig::FileReaderConfig(_config) = _config {
            config.set(_config);
        }
    });

    let update_watcher_path = move |event: rt::Event| {
        event.prevent_default();
        spawn_local_scoped(cx, async move {
            match pick_folder().await {
                None => (),
                Some(path) => (*config.modify()).watcher_path = path
            }
        });
    };

    let watcher_path = create_memo(cx, || {
        format!("{}", (*config.get()).watcher_path.display())
    });

    create_effect(cx, move || {                    // Apply config when it is updated
        config.track();
        spawn_local_scoped(cx, async move {
            if *config.get() != empty_file_reader_config() {
                apply_acquisitor_config(
                    AcquisitorConfig::FileReaderConfig((*config.get()).clone())
                ).await;
            }
        });
    });

    let do_nothing = |event: rt::Event| event.prevent_default();

    view! { cx, 
        form(on:submit=do_nothing) {
            input(type="submit", style="display: none;")

            p(class="mini-title") {
                p { "Aquisitor" }
                p { "(Leitor de Arquivos) "}
            }

            div(class="element") {
                p { "Caminho para vigiar:" }
                p { 
                    button(on:click=update_watcher_path) { " " }
                    (watcher_path.get())
                }
            }

        }
    }
}

