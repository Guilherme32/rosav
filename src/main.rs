use sycamore::prelude::*;
// use itertools::Itertools;
// use std::iter;

use wasm_bindgen::prelude::*;
// use serde::{Serialize, Deserialize};
use sycamore::futures::spawn_local_scoped;
use serde_wasm_bindgen::{to_value, from_value};

use gloo_timers::future::TimeoutFuture;


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

#[component]
fn Graph<G:Html>(cx: Scope) -> View<G> {
    let is_ready = create_signal(cx, false);
    let path = create_signal(cx, String::new());

    spawn_local_scoped(cx, async move {
        loop {
            TimeoutFuture::new(200).await;
            if unread_spectrum().await {
                if let Some(spectrum_path) = get_last_spectrum_path().await {
                    path.set(spectrum_path);
                }
            }
            is_ready.set(unread_spectrum().await);
        }
    });

    view! { cx,
        div(class="graph-space") {
            p { "Grafico pronto: " (is_ready.get()) }
            svg(viewbox = "0 0 480 360",
                height=360,
                width=480)
            {
                path(d=path.get(), fill="none", stroke="#000000", stroke-width="3")
                path(d="M 1,1 L 479,1 L 479,359 L 1,359 L 1,1", fill="none",
                    stroke-width="2", stroke="#000000")
            }
        }
    }
}

#[component]
fn LowerBar<G:Html>(cx: Scope) -> View<G> {
    view! { cx, 
        div(class="lower-bar") {
            "icon 1"
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
            "SideBar"
        }
    }
}

#[component]
fn LogSpace<G:Html>(cx: Scope) -> View<G> {
    view! { cx,
        div(class="log-space") {
            p { "MensagemP" }
            p { "Mensagem" }
            p { "Mensagem" }
            p { "Mensagem" }
            p { "Mensagem" }
            p { "Mensagem" }
            p { "Mensagem" }
            p { "Mensagem" }
            p { "Mensagem" }
            p { "Mensagem" }
            p { "Mensagem" }
            p { "Mensagem" }
            p { "Mensagem" }
            p { "Mensagem" }
            p { "Mensagem" }
            p { "MensagemL" }
        }
    }
}