use gloo_timers::future::TimeoutFuture;
use sycamore::futures::spawn_local_scoped;
use sycamore::{prelude::*, rt};
use wasm_bindgen::prelude::wasm_bindgen;

use crate::api::*;
use crate::trace::*;
use crate::ActiveSide;
use acquisitors::*;

mod acquisitor_config_renders;
use acquisitor_config_renders::*;

#[derive(Prop)]
pub struct SideBarProps<'a> {
    traces: &'a Signal<Vec<Trace>>,
    saving: &'a Signal<bool>,
    active_side: &'a ReadSignal<ActiveSide>,
    limits_change_flag: &'a Signal<bool>,
}

#[component]
pub fn SideBar<'a, G: Html>(cx: Scope<'a>, props: SideBarProps<'a>) -> View<G> {
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
                        ConfigWindow(limits_change_flag=props.limits_change_flag)
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
    saving: &'a Signal<bool>,
}

async fn freeze_callback(id: u8, traces_list: &Signal<Vec<Trace>>) {
    let mut traces_list = traces_list.modify();

    let trace = &mut traces_list[id as usize];
    if trace.svg_path.is_empty() {
        // Nao pode congelar onde não tem espectro
        return;
    }

    trace.freeze_time = Some(get_time().await);
    trace.active = false;

    let visible = trace.visible;
    let draw_valleys = trace.draw_valleys;
    let draw_valleys_mean = trace.draw_valleys_mean;

    traces_list.push(new_trace(id + 1, visible, draw_valleys, draw_valleys_mean));

    freeze_spectrum().await;
}

async fn delete_callback(id: u8, traces_list: &Signal<Vec<Trace>>) {
    traces_list.modify().remove(id as usize);

    for (i, trace) in traces_list.modify().iter_mut().enumerate() {
        trace.id = i as u8;
    }

    delete_frozen_spectrum(id as usize).await;
}

async fn visibility_callback(id: u8, traces_list: &Signal<Vec<Trace>>) {
    let trace = &mut traces_list.modify()[id as usize];
    trace.visible = !trace.visible;
}

async fn save_frozen_callback(id: u8, _traces_list: &Signal<Vec<Trace>>) {
    save_frozen_spectrum(id as usize).await;
}

async fn save_continuous_callback(saving: &Signal<bool>) {
    save_continuous(!*saving.get()).await;
    saving.set(get_saving().await);
}

async fn draw_valleys_callback(id: u8, traces_list: &Signal<Vec<Trace>>) {
    let trace = &mut traces_list.modify()[id as usize];
    trace.draw_valleys = !trace.draw_valleys;
}

async fn draw_valleys_mean_callback(id: u8, traces_list: &Signal<Vec<Trace>>) {
    let trace = &mut traces_list.modify()[id as usize];
    trace.draw_valleys_mean = !trace.draw_valleys_mean;
}

async fn change_color_callback(id: u8, traces_list: &Signal<Vec<Trace>>) {
    let trace = &mut traces_list.modify()[id as usize];
    trace.change_color();
}

#[component]
fn RenderTrace<'a, G: Html>(cx: Scope<'a>, props: RenderTraceProps<'a>) -> View<G> {
    let click_freeze = move |_| {
        spawn_local_scoped(cx, async move {
            freeze_callback(props.trace.id, props.traces_list).await;
        })
    };
    let click_delete = move |_| {
        spawn_local_scoped(cx, async move {
            delete_callback(props.trace.id, props.traces_list).await;
        })
    };
    let click_visibility = move |_| {
        spawn_local_scoped(cx, async move {
            visibility_callback(props.trace.id, props.traces_list).await;
        })
    };
    let click_save_frozen = move |_| {
        spawn_local_scoped(cx, async move {
            save_frozen_callback(props.trace.id, props.traces_list).await;
        })
    };
    let click_save_continuous = move |_| {
        spawn_local_scoped(cx, async move {
            save_continuous_callback(props.saving).await;
        })
    };
    let click_draw_valleys = move |_| {
        spawn_local_scoped(cx, async move {
            draw_valleys_callback(props.trace.id, props.traces_list).await;
        })
    };
    let click_draw_valleys_mean = move |_| {
        spawn_local_scoped(cx, async move {
            draw_valleys_mean_callback(props.trace.id, props.traces_list).await;
        })
    };
    let click_change_color = move |_| {
        spawn_local_scoped(cx, async move {
            change_color_callback(props.trace.id, props.traces_list).await;
        })
    };

    let name_style = props.trace.style();
    view! { cx,
        div(class="trace") {
            span(class="name", style=name_style) {
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
                    true => view! { cx, button(on:click=click_freeze, title="Congelar traço") { " " } },
                    false => view! { cx, button(on:click=click_delete, title="Excluir traço") { "󰜺 " } }
                })

                (if props.trace.visible {
                    view! { cx, button(on:click=click_visibility, title="Esconder traço") { "󰈈 " } }
                } else {
                    view! { cx, button(on:click=click_visibility, title="Revelar traço") { "󰈉 " } }
                })

                (if props.trace.draw_valleys {
                    view! { cx, button(on:click=click_draw_valleys, title="Esconder vales") { "󰆤 " } }
                } else {
                    view! { cx, button(on:click=click_draw_valleys, title="Revelar vales") { "󰽅 " } }
                })

                (if props.trace.active {
                    if *props.saving.get() {
                        view! { cx, button(on:click=click_save_continuous, title="Parar de salvar novos traços") { "󱃩 " } }
                    } else {
                        view! { cx, button(on:click=click_save_continuous, title="Salvar novos traços") { "󱧹 " } }
                    }
                } else {
                    view! { cx, button(on:click=click_save_frozen, title="Salvar traço") { " " } }
                })

                button(on:click=click_change_color, title="Mudar cor do traço") { "󰈊 " }

                (if props.trace.draw_valleys_mean {
                    view! { cx, button(on:click=click_draw_valleys_mean, title="Esconder médias") { "󰍐 " } }
                } else {
                    view! { cx, button(on:click=click_draw_valleys_mean, title="Revelar médias") { "󰍑 " } }
                })
            }
        }
    }
}

async fn hide_all_traces(traces_list: &Signal<Vec<Trace>>) {
    for mut trace in (*traces_list.modify()).iter_mut() {
        trace.visible = false;
    }
}

async fn show_all_traces(traces_list: &Signal<Vec<Trace>>) {
    for mut trace in (*traces_list.modify()).iter_mut() {
        trace.visible = true;
    }
}

async fn hide_all_valleys(traces_list: &Signal<Vec<Trace>>) {
    for mut trace in (*traces_list.modify()).iter_mut() {
        trace.draw_valleys = false;
    }
}

async fn show_all_valleys(traces_list: &Signal<Vec<Trace>>) {
    for mut trace in (*traces_list.modify()).iter_mut() {
        trace.draw_valleys = true;
    }
}

async fn hide_all_means(traces_list: &Signal<Vec<Trace>>) {
    for mut trace in (*traces_list.modify()).iter_mut() {
        trace.draw_valleys_mean = false;
    }
}

async fn show_all_means(traces_list: &Signal<Vec<Trace>>) {
    for mut trace in (*traces_list.modify()).iter_mut() {
        trace.draw_valleys_mean = true;
    }
}

#[derive(Prop)]
struct GlobalTraceButtons<'a> {
    traces: &'a Signal<Vec<Trace>>,
}

#[component]
fn GlobalTraceButtons<'a, G: Html>(cx: Scope<'a>, props: GlobalTraceButtons<'a>) -> View<G> {
    let click_hide_all_traces = move |_| {
        spawn_local_scoped(cx, async move {
            hide_all_traces(props.traces).await;
        })
    };
    let click_show_all_traces = move |_| {
        spawn_local_scoped(cx, async move {
            show_all_traces(props.traces).await;
        })
    };
    let click_hide_all_valleys = move |_| {
        spawn_local_scoped(cx, async move {
            hide_all_valleys(props.traces).await;
        })
    };
    let click_show_all_valleys = move |_| {
        spawn_local_scoped(cx, async move {
            show_all_valleys(props.traces).await;
        })
    };
    let click_hide_all_means = move |_| {
        spawn_local_scoped(cx, async move {
            hide_all_means(props.traces).await;
        })
    };
    let click_show_all_means = move |_| {
        spawn_local_scoped(cx, async move {
            show_all_means(props.traces).await;
        })
    };

    view! { cx,
        div(class="global-buttons") {
            button(on:click=click_hide_all_traces, title="Esconder todos os traços") { "󰈉 " }
            button(on:click=click_show_all_traces, title="Revelar todos os traços") { "󰈈 " }
            button(on:click=click_hide_all_valleys, title="Esconder todos os vales") { "󰽅 " }
            button(on:click=click_show_all_valleys, title="Revelar todos os vales") { "󰆤 " }
            button(on:click=click_hide_all_means, title="Esconder todas as médias") { "󰍑 " }
            button(on:click=click_show_all_means, title="Revelar todas as médias") { "󰍐 " }
            button(title="Salvar todos os traços") { " " }
        }
    }
}

#[derive(Prop)]
struct SideBarMainProps<'a> {
    traces: &'a Signal<Vec<Trace>>,
    saving: &'a Signal<bool>,
}

#[component]
fn SideBarMain<'a, G: Html>(cx: Scope<'a>, props: SideBarMainProps<'a>) -> View<G> {
    view! { cx,
        div(class="side-bar-main") {
            p(class="title") { "Traços" }

            GlobalTraceButtons(traces=props.traces)

            div(class="side-container back") {
                Keyed(        // Only re-renders on key change
                    iterable = props.traces,
                    view = move |cx, trace| view! {
                        cx, RenderTrace(
                            trace=trace,
                            traces_list=props.traces,
                            saving=props.saving
                        )
                    },
                    key = trace_info_identifier
                )
            }
        }
    }
}

fn trace_info_identifier(trace: &Trace) -> u64 {
    let active: u64 = trace.active as u64;
    let visible: u64 = (trace.visible as u64) * (1 << 1);
    let draw_valleys: u64 = (trace.draw_valleys as u64) * (1 << 2);
    let draw_valleys_mean: u64 = (trace.draw_valleys_mean as u64) * (1 << 3);
    let id: u64 = (trace.id as u64) * (1 << 4);
    let color_id: u64 = (trace.color_id.unwrap_or(255) as u64) * (1 << 12);

    active + visible + draw_valleys + draw_valleys_mean + id + color_id
}

#[component]
fn LogSpace<G: Html>(cx: Scope) -> View<G> {
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

#[derive(Prop)]
struct ConfigWindowProps<'a> {
    limits_change_flag: &'a Signal<bool>,
}

#[component]
fn ConfigWindow<'a, G: Html>(cx: Scope<'a>, props: ConfigWindowProps<'a>) -> View<G> {
    let handler_config = create_signal(cx, empty_handler_config());

    view! { cx,
        div(class="side-bar-main") {
            div(class="side-container back config") {
                p(class="title") { "Configurações" }
                RenderHandlerConfig (config=handler_config, limits_change_flag=props.limits_change_flag)
                RenderAcquisitorConfig (handler_config=handler_config)
            }
        }
    }
}

#[wasm_bindgen(inline_js = "export function blur() { document.activeElement.blur(); }")]
extern "C" {
    fn blur();
}

async fn get_old_handler_config<'a>(
    wl_min: &Signal<String>,
    wl_max: &Signal<String>,
    pwr_min: &Signal<String>,
    pwr_max: &Signal<String>,
    prominence: &Signal<String>,
    valley_detection: &Signal<String>,
    acquisitor: &Signal<String>,
) -> HandlerConfig {
    // Also updates on every field
    let _config = get_handler_config().await;

    if let Some(wl_limits) = _config.wavelength_limits {
        // Update wl limits input
        wl_min.set(format!("{:.1}", wl_limits.0 * 1e9));
        wl_max.set(format!("{:.1}", wl_limits.1 * 1e9));
    } else {
        wl_min.set("".to_string());
        wl_max.set("".to_string());
    }

    if let Some(pwr_limits) = _config.power_limits {
        // Update pwr limits input
        pwr_min.set(format!("{:.2}", pwr_limits.0));
        pwr_max.set(format!("{:.2}", pwr_limits.1));
    } else {
        pwr_min.set("".to_string());
        pwr_max.set("".to_string());
    }

    match _config.acquisitor {
        AcquisitorSimple::FileReader => acquisitor.set("file_reader".to_string()),
        AcquisitorSimple::Imon => acquisitor.set("imon".to_string()),
    }

    let _prominence = match _config.valley_detection {
        ValleyDetection::None => {
            valley_detection.set("none".to_string());
            3.0
        }
        ValleyDetection::Simple { prominence } => {
            valley_detection.set("simple".to_string());
            prominence
        }
        ValleyDetection::Lorentz { prominence } => {
            valley_detection.set("lorentz".to_string());
            prominence
        }
    };

    prominence.set(_prominence.to_string());

    _config
}

#[derive(Prop)]
struct HandlerConfigProps<'a> {
    config: &'a Signal<HandlerConfig>,
    limits_change_flag: &'a Signal<bool>,
}

#[component]
fn RenderHandlerConfig<'a, G: Html>(cx: Scope<'a>, props: HandlerConfigProps<'a>) -> View<G> {
    let wl_min = create_signal(cx, String::new());
    let wl_max = create_signal(cx, String::new());

    let pwr_min = create_signal(cx, String::new());
    let pwr_max = create_signal(cx, String::new());

    let prominence = create_signal(cx, String::new());
    let valley_detection = create_signal(cx, String::new());

    let acquisitor = create_signal(cx, String::new());

    spawn_local_scoped(cx, async move {
        let _config = get_old_handler_config(
            wl_min,
            wl_max,
            pwr_min,
            pwr_max,
            prominence,
            valley_detection,
            acquisitor,
        )
        .await;
        props.config.set(_config); // Update whole config
    });

    let unfocus = move |event: rt::Event| {
        blur();
        event.prevent_default();
    };

    let update_save_path = move |_| {
        spawn_local_scoped(cx, async move {
            match pick_folder().await {
                None => (),
                Some(path) => (props.config.modify()).auto_save_path = path,
            }
        });
    };

    let save_path = create_memo(cx, || {
        ((props.config.get()).auto_save_path.display()).to_string()
    });

    let update_limits = move |_| {
        update_wavelength_limits(wl_min, wl_max, props.config);
        update_power_limits(pwr_min, pwr_max, props.config);
    };

    let acquisitor_select = move |_| {
        blur();
        match (*acquisitor.get()).as_str() {
            "file_reader" => (props.config.modify()).acquisitor = AcquisitorSimple::FileReader,
            "imon" => (props.config.modify()).acquisitor = AcquisitorSimple::Imon,
            _ => (),
        }
    };

    let valley_detection_select = move |_| {
        blur();
        let prominence_result = prominence.get().parse::<f64>();
        match prominence_result {
            Ok(prominence) => match (*valley_detection.get()).as_str() {
                "none" => (props.config.modify()).valley_detection = ValleyDetection::None,
                "simple" => {
                    (props.config.modify()).valley_detection =
                        ValleyDetection::Simple { prominence }
                }
                "lorentz" => {
                    (props.config.modify()).valley_detection =
                        ValleyDetection::Lorentz { prominence }
                }
                _ => (),
            },
            Err(_) => {
                prominence.set("3".to_string());
            }
        }
    };

    create_effect(cx, move || {
        // Apply config when it is updated
        props.config.track();
        spawn_local_scoped(cx, async move {
            if *props.config.get() != empty_handler_config() {
                apply_handler_config((*props.config.get()).clone()).await;
            }
        });
    });

    create_effect(cx, move || {
        props.limits_change_flag.track();
        spawn_local_scoped(cx, async move {
            if *props.limits_change_flag.get() {
                let _config = get_old_handler_config(
                    wl_min,
                    wl_max,
                    pwr_min,
                    pwr_max,
                    prominence,
                    valley_detection,
                    acquisitor,
                )
                .await;
                props.config.set(_config); // Update whole config on zoom
                props.limits_change_flag.set(false);
            }
        });
    });

    view! { cx,
        form(class="side-container back config", on:submit=unfocus) {
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
                p { "Detecção de vale:" }
                select(
                    name="valley_detection",
                    bind:value=valley_detection,
                    on:input=valley_detection_select
                ) {
                    option(value="none") { "Nenhuma" }
                    option(value="simple") { "Simples" }
                    option(value="lorentz") { "Lorentziana" }
                }
            }

            div(class="element") {
                p { "Proeminência mínima:"}
                p {
                    input(
                        bind:value=prominence,
                        on:input=|_| check_number_input(prominence),
                        on:focusout=valley_detection_select
                    ) {}
                    "(dB)"
                }
            }

            div(class="element") {
                p { "Tipo de aquisitor:" }            // TODO implementar mudança quando passar o outro aquisitor
                select(
                    name="acquisitor",
                    bind:value=acquisitor,
                    on:input=acquisitor_select
                ) {
                    option(value="file_reader") { "Leitor de arquivos" }
                    option(value="imon") { "Ibsen IMON" }
                }
            }
        }
    }
}

fn update_wavelength_limits(
    wl_min: &ReadSignal<String>,
    wl_max: &ReadSignal<String>,
    config: &Signal<HandlerConfig>,
) {
    let new_limits: Option<(f64, f64)> = if wl_min.get().len() == 0 || wl_max.get().len() == 0 {
        None
    } else {
        let min_float = wl_min.get().parse::<f64>();
        let max_float = wl_max.get().parse::<f64>();

        match (min_float, max_float) {
            (Ok(min), Ok(max)) => Some((min * 1e-9, max * 1e-9)),
            (_, _) => None,
        }
    };

    if new_limits != config.get().wavelength_limits {
        config.modify().wavelength_limits = new_limits;
    }
}

fn update_power_limits(
    pwr_min: &ReadSignal<String>,
    pwr_max: &ReadSignal<String>,
    config: &Signal<HandlerConfig>,
) {
    let new_limits: Option<(f64, f64)> = if pwr_min.get().len() == 0 || pwr_max.get().len() == 0 {
        None
    } else {
        let min_float = pwr_min.get().parse::<f64>();
        let max_float = pwr_max.get().parse::<f64>();

        match (min_float, max_float) {
            (Ok(min), Ok(max)) => Some((min, max)),
            (_, _) => None,
        }
    };

    if new_limits != config.get().power_limits {
        config.modify().power_limits = new_limits;
    }
}

fn check_number_input(input: &Signal<String>) {
    let mut temp_copy = (*input.get()).clone();
    temp_copy.push('1');
    if temp_copy.parse::<f64>().is_err() {
        input.set(String::new());
    }
}

#[derive(Prop)]
struct AcquisitorConfigProps<'a> {
    handler_config: &'a Signal<HandlerConfig>,
}

#[component]
fn RenderAcquisitorConfig<'a, G: Html>(cx: Scope<'a>, props: AcquisitorConfigProps<'a>) -> View<G> {
    view! { cx,
        (match props.handler_config.get().acquisitor {
            AcquisitorSimple::FileReader => view! { cx,
                RenderFileReaderConfig {}
            },
            AcquisitorSimple::Imon => view! { cx,
                RenderImonConfig {}
            }
        })
    }
}
