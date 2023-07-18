use std::panic;

use sycamore::prelude::*;
// use itertools::Itertools;
// use std::iter;

use sycamore::futures::{spawn_local, spawn_local_scoped};

use gloo_timers::future::TimeoutFuture;

pub mod api;
use api::*;

pub mod trace;
use trace::*;

pub mod graph;
use graph::*;

pub mod side_bar;
use side_bar::*;

fn main() {
    panic::set_hook(Box::new(|reason| {
        let reason = format!("PANIC!!!! -> {}", reason);
        spawn_local(async move {
            print_backend(&reason).await;
        });
    }));

    sycamore::render(|cx| {
        view! { cx,
            Main {}
        }
    })
}

async fn get_trace_info() -> TraceInfo {
    let svg_size = get_svg_size().await;
    let wavelength_limits = get_wavelength_limits().await;
    let power_limits = get_power_limits().await;

    TraceInfo {
        svg_size,
        wavelength_limits,
        power_limits,
    }
}

pub enum ActiveSide {
    Traces,
    Config,
}

// COMPONENTS ----------------------------

#[component]
fn Main<G: Html>(cx: Scope) -> View<G> {
    let traces = create_signal(cx, vec![new_trace(0)]);

    let svg_size = create_signal(cx, (0i32, 0i32));

    spawn_local_scoped(cx, async move {
        loop {
            TimeoutFuture::new(200).await; // 5 fps, #TODO send to config
            let current_info = get_trace_info().await;

            if unread_spectrum().await {
                // Get the latest spectrum if it is available
                let new_path = get_last_spectrum_path().await;

                traces.modify().last_mut().map(|trace| {
                    trace.drawn_info = current_info.clone();
                    trace.svg_path = new_path;
                });
                continue; // Skip the loop to end the modify() and avoid problems
            }

            let new_svg_size = get_svg_size().await;
            for (id, trace) in traces.modify().iter_mut().enumerate() {
                // Update when the window changes
                if trace.svg_path.len() == 0 {
                    // No old spectrum, no update
                    continue;
                }
                if trace.drawn_info != current_info {
                    trace.drawn_info = current_info.clone();
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

    let saving = create_signal(cx, false);
    let connection_state = create_signal(cx, ConnectionState::Disconnected);

    spawn_local_scoped(cx, async move {
        loop {
            TimeoutFuture::new(200).await; // 5 fps, #TODO passar pra configsend to
            update_state(connection_state).await;
        }
    });

    let active_side = create_signal(cx, ActiveSide::Traces);

    view! { cx,
        div(class="horizontal-container") {

            SideBar(
                traces=traces,
                saving=saving,
                active_side=active_side
            )

            div(class="vertical-container") {
                Graph(
                    traces=traces,
                    svg_size=svg_size
                )

                LowerBar(
                    saving=saving,
                    connection_state=connection_state,
                    active_side=active_side
                )
            }
        }
    }
}

async fn update_state<'a>(connection_state: &'a Signal<ConnectionState>) {
    if let Some(new_state) = get_connection_state().await {
        if new_state != *connection_state.get() {
            connection_state.set(new_state);
        }
    }
}

#[derive(Prop)]
struct LowerBarProps<'a> {
    saving: &'a ReadSignal<bool>,
    connection_state: &'a Signal<ConnectionState>,
    active_side: &'a Signal<ActiveSide>,
}

#[component]
fn LowerBar<'a, G: Html>(cx: Scope<'a>, props: LowerBarProps<'a>) -> View<G> {
    let connect = move |_| {
        spawn_local_scoped(cx, async move {
            connect_acquisitor().await;
            update_state(props.connection_state).await;
        })
    };
    let disconnect = move |_| {
        spawn_local_scoped(cx, async move {
            disconnect_acquisitor().await;
            update_state(props.connection_state).await;
        })
    };
    let start_reading = move |_| {
        spawn_local_scoped(cx, async move {
            acquisitor_start_reading().await;
            update_state(props.connection_state).await;
        })
    };
    let stop_reading = move |_| {
        spawn_local_scoped(cx, async move {
            acquisitor_stop_reading().await;
            update_state(props.connection_state).await;
        })
    };

    let enter_config = move |_| match *props.active_side.get() {
        ActiveSide::Config => (),
        _ => {
            props.active_side.set(ActiveSide::Config);
        }
    };

    let enter_traces = move |_| match *props.active_side.get() {
        ActiveSide::Traces => (),
        _ => {
            props.active_side.set(ActiveSide::Traces);
        }
    };

    view! { cx,
        div(class="lower-bar back") {
            div() {
                button(on:click=enter_traces) { "󰽉 "}
                button(on:click=enter_config) { "󰢻 "}
            }
            div() {
                (match *props.connection_state.get() {
                    ConnectionState::Connected =>
                        view! { cx,
                            button(on:click=start_reading, class="no-offset") { " " }
                            // button(style="padding-right: 0.6rem;") { "󱑹 " }        // TODO put single read
                            button(on:click=disconnect) { "󱐤 " }
                        },
                    ConnectionState::Reading =>
                        view! { cx,
                            button(on:click=stop_reading, class="no-offset") { " " }
                            button(on:click=disconnect) { "󱐤 " }
                        },
                    ConnectionState::Disconnected =>
                        view! { cx,
                            button(on:click=connect, class="no-offset") { "󱐥 " }
                        }
                })
            }
            Status(saving=props.saving, connection_state=props.connection_state)
        }
    }
}

#[derive(Prop)]
struct StatusProps<'a> {
    saving: &'a ReadSignal<bool>,
    connection_state: &'a ReadSignal<ConnectionState>,
}

#[component]
fn Status<'a, G: Html>(cx: Scope<'a>, props: StatusProps<'a>) -> View<G> {
    view! { cx,
        div(class="status") {
            (match *props.connection_state.get() {
                ConnectionState::Connected =>
                    view! { cx, p() { "Conectado" } },
                ConnectionState::Disconnected =>
                    view! { cx, p() { "Desconectado" } },
                ConnectionState::Reading =>
                    view! { cx, p() { "Lendo Const." } }
            })

            (if *props.saving.get() {
                view! { cx,
                    p() { "Salvando" }
                }
            } else {
                view! { cx,
                    p() { "Não Salvando" }
                }
            })
        }
    }
}
