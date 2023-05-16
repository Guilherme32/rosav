use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};
use serde_wasm_bindgen::{to_value, from_value};
use std::fmt;

// API -------------------------------

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["window", "__TAURI__", "tauri"])]
    async fn invoke(cmd: &str, args: JsValue) -> JsValue;
}

pub async fn hello() {
    invoke("hello", to_value(&()).unwrap()).await;
}

#[derive(Serialize, Deserialize)]
struct PrintArgs<'a> {
    msg: &'a str
}
pub async fn print_backend<'a>(msg: &'a str) {
    invoke("print_backend", to_value(&PrintArgs { msg }).unwrap()).await;
}

pub async fn unread_spectrum() -> bool {
    let from_back = invoke("unread_spectrum", to_value(&()).unwrap()).await;
    let obj_rebuilt: bool = from_value(from_back).unwrap();

    obj_rebuilt
}

pub async fn get_last_spectrum_path() -> String {
    let from_back = invoke("get_last_spectrum_path", to_value(&()).unwrap()).await;
    let obj_rebuilt: String = from_value(from_back).unwrap();

    obj_rebuilt
}

// É i32 para poder fazer subtração, mas sempre será > 0 nos limites do programa
pub async fn get_window_size() -> (i32, i32) {
    let from_back = invoke("get_window_size", to_value(&()).unwrap()).await;
    let obj_rebuilt: (i32, i32) = from_value(from_back).unwrap();

    obj_rebuilt
}

pub async fn get_svg_size() -> (i32, i32) {
    let from_back = invoke("get_svg_size", to_value(&()).unwrap()).await;
    let obj_rebuilt: (i32, i32) = from_value(from_back).unwrap();

    obj_rebuilt
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct Log {
    pub id: u32,
    pub msg: String,
    pub log_type: LogType
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub enum LogType {
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

pub async fn get_last_logs() -> Vec::<Log> {
    let from_back = invoke("get_last_logs", to_value(&()).unwrap()).await;
    let obj_rebuilt: Vec::<Log> = from_value(from_back).unwrap();

    obj_rebuilt
}

pub async fn get_wavelength_limits() -> (f64, f64) {
    let from_back = invoke("get_wavelength_limits", to_value(&()).unwrap()).await;
    let obj_rebuilt: (f64, f64) = from_value(from_back).unwrap();

    obj_rebuilt
}

pub async fn get_power_limits() -> (f64, f64) {
    let from_back = invoke("get_power_limits", to_value(&()).unwrap()).await;
    let obj_rebuilt: (f64, f64) = from_value(from_back).unwrap();

    obj_rebuilt
}

pub async fn get_time() -> String {
    let from_back = invoke("get_time", to_value(&()).unwrap()).await;
    let obj_rebuilt: String = from_value(from_back).unwrap();

    obj_rebuilt
}

