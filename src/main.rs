use sycamore::prelude::*;
// use itertools::Itertools;
// use std::iter;

use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};
use sycamore::futures::spawn_local_scoped;
use serde_wasm_bindgen::{to_value, from_value};

use gloo_timers::future::TimeoutFuture;
use std::fmt;


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


// API -------------------------------

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "tauri"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
}


async fn unread_spectrum() -> bool {
    let from_back = invoke("unread_spectrum", to_value(&()).unwrap()).await;
    let obj_rebuilt: bool = from_value(from_back).unwrap();

    obj_rebuilt
}

async fn get_last_spectrum_path() -> Option<String> {
    let from_back = invoke("get_last_spectrum_path", to_value(&()).unwrap()).await;
    let obj_rebuilt: Option<String> = from_value(from_back).unwrap();

    obj_rebuilt
}

// É i32 para poder fazer subtração, mas sempre será > 0 nos limites do programa
async fn get_window_size() -> (i32, i32) {
    let from_back = invoke("get_window_size", to_value(&()).unwrap()).await;
    let obj_rebuilt: (i32, i32) = from_value(from_back).unwrap();

    obj_rebuilt
}

async fn get_svg_size() -> (i32, i32) {
    let from_back = invoke("get_svg_size", to_value(&()).unwrap()).await;
    let obj_rebuilt: (i32, i32) = from_value(from_back).unwrap();

    obj_rebuilt
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
struct Log {
    id: u32,
    msg: String,
    log_type: LogType
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
enum LogType {
    Info,
    Warning,
    Error
}
impl fmt::Display for LogType {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            LogType::Info => write!(f, "info"),
            LogType::Warning => write!(f, "warning"),
            LogType::Error => write!(f, "error")
        }
    }
}

async fn get_last_logs() -> Vec::<Log> {
    let from_back = invoke("get_last_logs", to_value(&()).unwrap()).await;
    let obj_rebuilt: Vec::<Log> = from_value(from_back).unwrap();

    obj_rebuilt
}

// COMPONENTS ----------------------------

#[component]
fn Graph<G:Html>(cx: Scope) -> View<G> {
    let is_ready = create_signal(cx, false);
    let path = create_signal(cx, String::new());
    let svg_size = create_signal(cx, (0i32, 0i32));

    let path_sqr = create_memo(cx, || 
        format!("M 1,1 L {},1 L {},{} L 1,{} L 1,1",
            (*svg_size.get()).0 - 1,
            (*svg_size.get()).0 - 1, (*svg_size.get()).1 - 1,
            (*svg_size.get()).1 - 1
        )
    );

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
            // div(class="placeholder") {
            //     p { "Área do gráfico" }
            //     p { "Sem espectro para mostrar" }
            //     p { "Grafico pronto: " (is_ready.get()) }
            //     p { "Window size: " }
            //     p { "x: " (window_size.get().0) }
            //     p { "y: " (window_size.get().1) }
            //     p { "svg x: " (svg_size.get().0) }
            //     p { "svg y: " (svg_size.get().1) }
            // }
            svg(
                width=svg_size.get().0,
                height=svg_size.get().1)
            {
                path(d=path.get(), fill="none", stroke="#000000", stroke-width="3")
                path(d=path_sqr.get(), fill="none",
                    stroke-width="2", stroke="#000000")
            }
        }
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

#[component]
fn SideBarMain<G:Html>(cx: Scope) -> View<G> {
    view! { cx,
        div(class="side-bar-main") {
            p(class="title") { "Traços" }

            div(class="trace-container back") {
                div(class="trace") {
                    span(class="name") { "A" }
                    span(class="status") { "(10:24)" }
                    div(class="buttons") {
                        button() { "󰜺 " }
                        button() { " " }
                        button() { " " }
                        button() { "⚡" }
                    }
                }
                div(class="trace") {
                    span(class="name") { "A" }
                    span(class="status") { "(10:24)" }
                    div(class="buttons") {
                        button() { "󰜺 " }
                        button() { " " }
                        button() { " " }
                        button() { "⚡" }
                    }
                }
                div(class="trace") {
                    span(class="name") { "B" }
                    span(class="status") { "(10:24)" }
                    div(class="buttons") {
                        button() { "󰜺 " }
                        button() { " " }
                        button() { " " }
                        button() { "⚡" }
                    }
                }
                div(class="trace") {
                   span(class="name") { "C" }
                    span(class="status") { "(10:24)" }
                    div(class="buttons") {
                        button() { "󰜺 " }
                        button() { " " }
                        button() { " " }
                        button() { "⚡" }
                    }
                }
                div(class="trace") {
                   span(class="name") { "D" }
                    span(class="status") { "(10:24)" }
                    div(class="buttons") {
                        button() { "󰜺 " }
                        button() { " " }
                        button() { " " }
                        button() { "⚡" }
                    }
                }
                div(class="trace") {
                    span(class="name") { "A" }
                    span(class="status") { "(10:24)" }
                    div(class="buttons") {
                        button() { "󰜺 " }
                        button() { " " }
                        button() { " " }
                        button() { "⚡" }
                    }
                }
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