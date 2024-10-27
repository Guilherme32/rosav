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
    draw_shadow: &'a Signal<bool>,
    draw_time_series: &'a Signal<bool>,
    series_total_time: &'a Signal<i32>,
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
                            saving=props.saving,
                            draw_shadow=props.draw_shadow,
                            draw_time_series=props.draw_time_series,
                        )
                    },
                ActiveSide::Config =>
                    view! { cx,
                        ConfigWindow(
                        limits_change_flag=props.limits_change_flag,
                        series_total_time=props.series_total_time
                    )
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

    let trace = new_trace(trace); // new active trace
    traces_list.push(trace);

    freeze_spectrum().await;
}

async fn delete_callback(id: u8, traces_list: &Signal<Vec<Trace>>) {
    let mut traces_list = traces_list.modify();

    traces_list.remove(id as usize);

    for (i, trace) in traces_list.iter_mut().enumerate() {
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

// Group callbacks ------------------------------------------------------------
fn get_grouped_trace_ids(main_id: u8, traces_list: &Signal<Vec<Trace>>) -> Vec<u8> {
    let traces_list = traces_list.get();

    let group_id = if let Some(color_id) = traces_list[main_id as usize].color_id {
        color_id
    } else {
        return vec![];
    };

    traces_list
        .iter()
        .filter(|trace| {
            if let Some(color_id) = trace.color_id {
                color_id == group_id
            } else {
                false
            }
        })
        .filter(|trace| !trace.active)
        .map(|trace| trace.id)
        .collect()
}

async fn delete_group_callback(main_id: u8, traces_list: &Signal<Vec<Trace>>) {
    let deleted_ids = get_grouped_trace_ids(main_id, traces_list);
    let mut traces_list = traces_list.modify();

    for id in deleted_ids.iter().rev() {
        traces_list.remove(*id as usize);
        delete_frozen_spectrum(*id as usize).await;
    }

    for (i, trace) in traces_list.iter_mut().enumerate() {
        trace.id = i as u8;
    }
}

async fn visibility_group_callback(main_id: u8, traces_list: &Signal<Vec<Trace>>) {
    let new_visible = !traces_list.get()[main_id as usize].visible;
    let group_ids = get_grouped_trace_ids(main_id, traces_list);
    let mut traces_list = traces_list.modify();

    for id in group_ids {
        traces_list[id as usize].visible = new_visible;
    }
}

async fn draw_valleys_group_callback(main_id: u8, traces_list: &Signal<Vec<Trace>>) {
    let new_draw = !traces_list.get()[main_id as usize].draw_valleys;
    let group_ids = get_grouped_trace_ids(main_id, traces_list);
    let mut traces_list = traces_list.modify();

    for id in group_ids {
        traces_list[id as usize].draw_valleys = new_draw;
    }
}

async fn draw_valleys_mean_group_callback(main_id: u8, traces_list: &Signal<Vec<Trace>>) {
    let new_draw = !traces_list.get()[main_id as usize].draw_valleys_mean;
    let group_ids = get_grouped_trace_ids(main_id, traces_list);
    let mut traces_list = traces_list.modify();

    for id in group_ids {
        traces_list[id as usize].draw_valleys_mean = new_draw;
    }
}

async fn reset_color_callback(id: u8, traces_list: &Signal<Vec<Trace>>) {
    let trace = &mut traces_list.modify()[id as usize];
    trace.reset_color();
}

#[component]
fn RenderTrace<'a, G: Html>(cx: Scope<'a>, props: RenderTraceProps<'a>) -> View<G> {
    // Left clicks ------------------------------------------------------------
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

    // Right clicks -----------------------------------------------------------
    let click_group_delete = move |_| {
        spawn_local_scoped(cx, async move {
            delete_group_callback(props.trace.id, props.traces_list).await;
        })
    };
    let click_group_visibility = move |_| {
        spawn_local_scoped(cx, async move {
            visibility_group_callback(props.trace.id, props.traces_list).await;
        })
    };
    let click_group_draw_valleys = move |_| {
        spawn_local_scoped(cx, async move {
            draw_valleys_group_callback(props.trace.id, props.traces_list).await;
        })
    };
    let click_group_draw_valleys_mean = move |_| {
        spawn_local_scoped(cx, async move {
            draw_valleys_mean_group_callback(props.trace.id, props.traces_list).await;
        })
    };
    let click_reset_color = move |_| {
        spawn_local_scoped(cx, async move {
            reset_color_callback(props.trace.id, props.traces_list).await;
        })
    };

    // Actual render ----------------------------------------------------------
    let name_style = props.trace.name_style();
    let group_style = props.trace.group_style();

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
                    true => view! { cx,
                        button(on:click=click_freeze,
                            title="Congelar traço"
                        ) { " " } },
                    false => view! { cx, button(
                        on:click=click_delete,
                        on:contextmenu=click_group_delete,
                        title="Excluir traço"
                    ) { "󰜺 " } }
                })

                (if props.trace.visible {
                    view! { cx, button(
                        on:click=click_visibility,
                        on:contextmenu=click_group_visibility,
                        title="Esconder traço"
                    ) { "󰈈 " } }
                } else {
                    view! { cx, button(
                        on:click=click_visibility,
                        on:contextmenu=click_group_visibility,
                        title="Revelar traço"
                    ) { "󰈉 " } }
                })

                (if props.trace.draw_valleys {
                    view! { cx, button(
                        on:click=click_draw_valleys,
                        on:contextmenu=click_group_draw_valleys,
                        title="Esconder vales/picos",
                    ) { "󰆤 " } }
                } else {
                    view! { cx, button(
                        on:click=click_draw_valleys,
                        on:contextmenu=click_group_draw_valleys,
                        title="Revelar vales/picos"
                    ) { "󰽅 " } }
                })

                (if props.trace.active {
                    if *props.saving.get() {
                        view! { cx, button(
                            on:click=click_save_continuous,
                            title="Parar de salvar novos traços"
                        ) { "󱃩 " } }
                    } else {
                        view! { cx, button(
                            on:click=click_save_continuous,
                            title="Salvar novos traços"
                        ) { "󱧹 " } }
                    }
                } else {
                    view! { cx, button(
                            on:click=click_save_frozen,
                            title="Salvar traço"
                        ) { " " } }
                })

                button(
                    on:click=click_change_color,
                    style=group_style,
                    on:contextmenu=click_reset_color,
                    title="Mudar grupo do traço"
                ) { "󰈊 " }

                (if props.trace.draw_valleys_mean {
                    view! { cx, button(
                        on:click=click_draw_valleys_mean,
                        on:contextmenu=click_group_draw_valleys_mean,
                        title="Esconder médias"
                    ) { "󰍐 " } }
                } else {
                    view! { cx, button(
                        on:click=click_draw_valleys_mean,
                        on:contextmenu=click_group_draw_valleys_mean,
                        title="Revelar médias"
                    ) { "󰍑 " } }
                })
            }
        }
    }
}

async fn hide_all_traces(traces_list: &Signal<Vec<Trace>>) {
    for trace in (*traces_list.modify()).iter_mut() {
        trace.visible = false;
    }
}

async fn show_all_traces(traces_list: &Signal<Vec<Trace>>) {
    for trace in (*traces_list.modify()).iter_mut() {
        trace.visible = true;
    }
}

async fn hide_all_valleys(traces_list: &Signal<Vec<Trace>>) {
    for trace in (*traces_list.modify()).iter_mut() {
        trace.draw_valleys = false;
    }
}

async fn show_all_valleys(traces_list: &Signal<Vec<Trace>>) {
    for trace in (*traces_list.modify()).iter_mut() {
        trace.draw_valleys = true;
    }
}

async fn hide_all_means(traces_list: &Signal<Vec<Trace>>) {
    for trace in (*traces_list.modify()).iter_mut() {
        trace.draw_valleys_mean = false;
    }
}

async fn show_all_means(traces_list: &Signal<Vec<Trace>>) {
    for trace in (*traces_list.modify()).iter_mut() {
        trace.draw_valleys_mean = true;
    }
}

async fn toggle_shadow(draw_shadow: &Signal<bool>) {
    draw_shadow.set(!*draw_shadow.get());
}

async fn toggle_time_series(draw_time_series: &Signal<bool>) {
    draw_time_series.set(!*draw_time_series.get());
}

#[derive(Prop)]
struct GlobalTraceButtons<'a> {
    traces: &'a Signal<Vec<Trace>>,
    draw_shadow: &'a Signal<bool>,
    draw_time_series: &'a Signal<bool>,
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
    let click_save_all_spectra = move |_| {
        spawn_local_scoped(cx, async move {
            save_all_spectra().await;
        })
    };
    let click_toggle_shadow = move |_| {
        spawn_local_scoped(cx, async move {
            toggle_shadow(props.draw_shadow).await;
        })
    };
    let click_toggle_time_series = move |_| {
        spawn_local_scoped(cx, async move {
            toggle_time_series(props.draw_time_series).await;
        })
    };

    view! { cx,
        div(class="global-buttons back") {
            button(on:click=click_hide_all_traces, title="Esconder todos os traços") { "󰈉 " }
            button(on:click=click_show_all_traces, title="Revelar todos os traços") { "󰈈 " }
            button(on:click=click_hide_all_valleys, title="Esconder todos os vales/picos") { "󰽅 " }
            button(on:click=click_show_all_valleys, title="Revelar todos os vales/picos") { "󰆤 " }
            button(on:click=click_hide_all_means, title="Esconder todas as médias") { "󰍑 " }
            button(on:click=click_show_all_means, title="Revelar todas as médias") { "󰍐 " }
            button(on:click=click_save_all_spectra, title="Salvar todos os traços") { " " }

            (if *props.draw_shadow.get() {
                view! {cx, button(on:click=click_toggle_shadow, title="Não desenhar sombra") { "󰊠 " } }
            } else {
                view! {cx, button(on:click=click_toggle_shadow, title="Desenhar sombra") { "󰧵 " } }
            })

            (if *props.draw_time_series.get() {
                view! {cx, button(
                    on:click=click_toggle_time_series,
                    title="Não desenhar série temporal"
                ) { "󰀠 " } }
            } else {
                view! {cx, button(
                        on:click=click_toggle_time_series,
                        title="Desenhar série temporal"
                    ) { "󰀣 " } }
            })
        }
    }
}

#[derive(Prop)]
struct SideBarMainProps<'a> {
    traces: &'a Signal<Vec<Trace>>,
    saving: &'a Signal<bool>,
    draw_shadow: &'a Signal<bool>,
    draw_time_series: &'a Signal<bool>,
}

#[component]
fn SideBarMain<'a, G: Html>(cx: Scope<'a>, props: SideBarMainProps<'a>) -> View<G> {
    view! { cx,
        div(class="side-bar-main") {
            p(class="title") { "Traços" }

            GlobalTraceButtons(
                traces=props.traces,
                draw_shadow=props.draw_shadow,
                draw_time_series=props.draw_time_series,
            )

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
    series_total_time: &'a Signal<i32>,
}

#[component]
fn ConfigWindow<'a, G: Html>(cx: Scope<'a>, props: ConfigWindowProps<'a>) -> View<G> {
    let handler_config = create_signal(cx, empty_handler_config());

    view! { cx,
        div(class="side-bar-main") {
            p(class="title") { "Configurações" }
            div(class="side-container back config") {
                RenderHandlerConfig(
                    config=handler_config,
                    limits_change_flag=props.limits_change_flag,
                    global_series_total_time=props.series_total_time
                )
                RenderAcquisitorConfig (handler_config=handler_config)
            }
        }
    }
}

#[wasm_bindgen(inline_js = "export function blur() { document.activeElement.blur(); }")]
extern "C" {
    fn blur();
}

struct OldHandlerSignals<'a> {
    wl_min: &'a Signal<String>,
    wl_max: &'a Signal<String>,
    pwr_min: &'a Signal<String>,
    pwr_max: &'a Signal<String>,
    valley_prominence: &'a Signal<String>,
    valley_detection: &'a Signal<String>,
    peak_prominence: &'a Signal<String>,
    peak_detection: &'a Signal<String>,
    shadow_length: &'a Signal<String>,
    acquisitor: &'a Signal<String>,
    series_draw_valleys: &'a Signal<bool>,
    series_draw_valley_means: &'a Signal<bool>,
    series_draw_peaks: &'a Signal<bool>,
    series_draw_peak_means: &'a Signal<bool>,
    series_total_time: &'a Signal<String>,
}

async fn get_old_handler_config(signals: OldHandlerSignals<'_>) -> HandlerConfig {
    // Also updates on every field
    let _config = get_handler_config().await;

    if let Some(wl_limits) = _config.wavelength_limits {
        // Update wl limits input
        signals.wl_min.set(format!("{:.1}", wl_limits.0 * 1e9));
        signals.wl_max.set(format!("{:.1}", wl_limits.1 * 1e9));
    } else {
        signals.wl_min.set("".to_string());
        signals.wl_max.set("".to_string());
    }

    if let Some(pwr_limits) = _config.power_limits {
        // Update pwr limits input
        signals.pwr_min.set(format!("{:.2}", pwr_limits.0));
        signals.pwr_max.set(format!("{:.2}", pwr_limits.1));
    } else {
        signals.pwr_min.set("".to_string());
        signals.pwr_max.set("".to_string());
    }

    let prominence = match _config.valley_detection {
        CriticalDetection::None => {
            signals.valley_detection.set("none".to_string());
            3.0
        }
        CriticalDetection::Simple { prominence } => {
            signals.valley_detection.set("simple".to_string());
            prominence
        }
        CriticalDetection::Lorentz { prominence } => {
            signals.valley_detection.set("lorentz".to_string());
            prominence
        }
    };

    signals.valley_prominence.set(prominence.to_string());

    let prominence = match _config.peak_detection {
        CriticalDetection::None => {
            signals.peak_detection.set("none".to_string());
            3.0
        }
        CriticalDetection::Simple { prominence } => {
            signals.peak_detection.set("simple".to_string());
            prominence
        }
        CriticalDetection::Lorentz { prominence } => {
            signals.peak_detection.set("lorentz".to_string());
            prominence
        }
    };

    signals.peak_prominence.set(prominence.to_string());

    signals.shadow_length.set(_config.shadow_length.to_string());

    signals
        .series_draw_valleys
        .set(_config.time_series_config.draw_valleys);
    signals
        .series_draw_valley_means
        .set(_config.time_series_config.draw_valley_means);
    signals
        .series_draw_peaks
        .set(_config.time_series_config.draw_peaks);
    signals
        .series_draw_peak_means
        .set(_config.time_series_config.draw_peak_means);
    signals
        .series_total_time
        .set(_config.time_series_config.total_time.to_string());

    match _config.acquisitor {
        AcquisitorSimple::FileReader => signals.acquisitor.set("file_reader".to_string()),
        AcquisitorSimple::Imon => signals.acquisitor.set("imon".to_string()),
        AcquisitorSimple::Example => signals.acquisitor.set("example".to_string()),
    }

    _config
}

fn unfocus(event: rt::Event) {
    blur();
    event.prevent_default();
}

#[derive(Prop)]
struct HandlerConfigProps<'a> {
    config: &'a Signal<HandlerConfig>,
    limits_change_flag: &'a Signal<bool>,
    global_series_total_time: &'a Signal<i32>,
}

// HOOOOOLY CRAP this component got out of hand!
// We need to break this down so we can actually read it
#[component]
fn RenderHandlerConfig<'a, G: Html>(cx: Scope<'a>, props: HandlerConfigProps<'a>) -> View<G> {
    // Init the signals -------------------------------------------------------
    let wl_min = create_signal(cx, String::new());
    let wl_max = create_signal(cx, String::new());

    let pwr_min = create_signal(cx, String::new());
    let pwr_max = create_signal(cx, String::new());

    let valley_prominence = create_signal(cx, String::new());
    let valley_detection = create_signal(cx, String::new());

    let peak_prominence = create_signal(cx, String::new());
    let peak_detection = create_signal(cx, String::new());

    let series_draw_valleys = create_signal(cx, false);
    let series_draw_valley_means = create_signal(cx, false);
    let series_draw_peaks = create_signal(cx, false);
    let series_draw_peak_means = create_signal(cx, false);
    let series_total_time = create_signal(cx, String::new());

    let shadow_length = create_signal(cx, String::new());

    let acquisitor = create_signal(cx, String::new());

    // Fetch the old config ---------------------------------------------------
    spawn_local_scoped(cx, async move {
        let old_handler_signals = OldHandlerSignals {
            wl_min,
            wl_max,
            pwr_min,
            pwr_max,
            valley_prominence,
            valley_detection,
            peak_prominence,
            peak_detection,
            shadow_length,
            acquisitor,
            series_draw_valleys,
            series_draw_valley_means,
            series_draw_peaks,
            series_draw_peak_means,
            series_total_time,
        };

        let _config = get_old_handler_config(old_handler_signals).await;
        props.config.set(_config); // Update whole config
    });

    // Create the callbacks ---------------------------------------------------
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

    let update_shadow_length = move |_| {
        if let Ok(length) = shadow_length.get().parse::<usize>() {
            (props.config.modify()).shadow_length = length;
        } else {
            let old_shadow_len = props.config.get().shadow_length.to_string();
            shadow_length.set(old_shadow_len);
        }
    };

    let update_series_total_time = move |_| {
        if let Ok(total_time) = series_total_time.get().parse::<u64>() {
            if (1..601).contains(&total_time) {
                (props.config.modify()).time_series_config.total_time = total_time;
                return;
            }
        }
        let old_total_time = props.config.get().time_series_config.total_time.to_string();
        series_total_time.set(old_total_time);
    };

    let acquisitor_select = move |_| {
        blur();
        match (*acquisitor.get()).as_str() {
            "file_reader" => (props.config.modify()).acquisitor = AcquisitorSimple::FileReader,
            "imon" => (props.config.modify()).acquisitor = AcquisitorSimple::Imon,
            "example" => (props.config.modify()).acquisitor = AcquisitorSimple::Example,
            _ => (),
        }
    };

    let valley_detection_select = move |_| {
        blur();
        let prominence_result = valley_prominence.get().parse::<f64>();
        match prominence_result {
            Ok(prominence) => match (*valley_detection.get()).as_str() {
                "none" => (props.config.modify()).valley_detection = CriticalDetection::None,
                "simple" => {
                    (props.config.modify()).valley_detection =
                        CriticalDetection::Simple { prominence }
                }
                "lorentz" => {
                    (props.config.modify()).valley_detection =
                        CriticalDetection::Lorentz { prominence }
                }
                _ => (),
            },
            Err(_) => {
                valley_prominence.set("3".to_string());
            }
        }
    };

    let peak_detection_select = move |_| {
        blur();
        let prominence_result = peak_prominence.get().parse::<f64>();
        match prominence_result {
            Ok(prominence) => match (*peak_detection.get()).as_str() {
                "none" => (props.config.modify()).peak_detection = CriticalDetection::None,
                "simple" => {
                    (props.config.modify()).peak_detection =
                        CriticalDetection::Simple { prominence }
                }
                "lorentz" => {
                    (props.config.modify()).peak_detection =
                        CriticalDetection::Lorentz { prominence }
                }
                _ => (),
            },
            Err(_) => {
                peak_prominence.set("3".to_string());
            }
        }
    };

    // Create effects ---------------------------------------------------------
    // Apply config when it is updated
    create_effect(cx, move || {
        props.config.track();
        spawn_local_scoped(cx, async move {
            if *props.config.get() != empty_handler_config() {
                let config = (*props.config.get()).clone();
                props
                    .global_series_total_time
                    .set(config.time_series_config.total_time as i32);
                apply_handler_config(config).await;
            }
        });
    });

    // Update if the config was changed elsewhere (as in the drag zoom)
    create_effect(cx, move || {
        props.limits_change_flag.track();
        spawn_local_scoped(cx, async move {
            if *props.limits_change_flag.get() {
                let old_handler_signals = OldHandlerSignals {
                    wl_min,
                    wl_max,
                    pwr_min,
                    pwr_max,
                    valley_prominence,
                    valley_detection,
                    peak_prominence,
                    peak_detection,
                    shadow_length,
                    acquisitor,
                    series_draw_valleys,
                    series_draw_valley_means,
                    series_draw_peaks,
                    series_draw_peak_means,
                    series_total_time,
                };

                let _config = get_old_handler_config(old_handler_signals).await;

                props.config.set(_config); // Update whole config on zoom
                props.limits_change_flag.set(false);
            }
        });
    });

    // Update when a checkbox changes state
    create_effect(cx, move || {
        let mut config = props.config.modify();
        config.time_series_config.draw_valleys = *series_draw_valleys.get();
        config.time_series_config.draw_peaks = *series_draw_peaks.get();
        config.time_series_config.draw_valley_means = *series_draw_valley_means.get();
        config.time_series_config.draw_peak_means = *series_draw_peak_means.get();
    });

    // Render it --------------------------------------------------------------
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
                p { "Limites do gráfico:"}
                p(class="spacer") {}
                p { "Comprimento de onda:"}
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
                p(class="spacer") {}
                p { "Potência:"}
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
                p { "Espectros na sombra: " }
                input(
                    bind:value=shadow_length,
                    type="number",
                    on:focusout=update_shadow_length
                ) {}
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

                p(class="spacer") {}

                p { "Proeminência mínima:"}
                p {
                    input(
                        bind:value=valley_prominence,
                        on:input=|_| check_number_input(valley_prominence),
                        on:focusout=valley_detection_select
                    ) {}
                    "(dB) (vale)"
                }
            }

            div(class="element") {
                p { "Detecção de pico:" }
                select(
                    name="peak_detection",
                    bind:value=peak_detection,
                    on:input=peak_detection_select
                ) {
                    option(value="none") { "Nenhuma" }
                    option(value="simple") { "Simples" }
                    option(value="lorentz") { "Lorentziana" }
                }

                p(class="spacer") {}

                p { "Proeminência mínima:"}
                p {
                    input(
                        bind:value=peak_prominence,
                        on:input=|_| check_number_input(peak_prominence),
                        on:focusout=peak_detection_select
                    ) {}
                    "(dB) (pico)"
                }
            }

            div(class="element") {
                p { "Série Temporal:"}
                p (class="spacer") {}
                p { "Desenhar: "}
                p {
                    label(class="check-container") {
                        input(
                            type="checkbox",
                            bind:checked=series_draw_valleys
                        ) {}
                        span(class="checkbox") {}
                        "Vales"
                    }
                    label(class="check-container") {
                        input(
                            type="checkbox",
                            bind:checked=series_draw_valley_means
                        ) {}
                        span(class="checkbox") {}
                        "Médias V."
                    }
                }
                p {
                    label(class="check-container") {
                        input(
                            type="checkbox",
                            bind:checked=series_draw_peaks
                        ) {}
                        span(class="checkbox") {}
                        "Picos"
                    }
                    label(class="check-container") {
                        input(
                            type="checkbox",
                            bind:checked=series_draw_peak_means
                        ) {}
                        span(class="checkbox") {}
                        "Médias P."
                    }
                }
                p (class="spacer") {}
                p {"Tempo total:"}
                p {
                    input(
                        type="number",
                        bind:value=series_total_time,
                        on:focusout=update_series_total_time,
                    ) {}
                    "(s)"
                }
            }

            div(class="element") {
                p { "Tipo de aquisitor:" }
                select(
                    name="acquisitor",
                    bind:value=acquisitor,
                    on:input=acquisitor_select
                ) {
                    option(value="file_reader") { "Leitor de arquivos" }
                    option(value="imon") { "Ibsen IMON" }
                    (
                        if cfg!(feature = "example") {
                            view!{cx, option(value="example") { "Aquisitor exemplo" } }
                        } else {
                            view!{cx, }
                        }
                    )
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
            },
            AcquisitorSimple::Example => view! { cx,
                RenderExampleConfig {}
            },
        })
    }
}
