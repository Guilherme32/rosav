use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};
use serde_wasm_bindgen::{to_value, from_value};
use std::fmt;

use std::path::PathBuf;

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
    // pub id: u32,
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

pub async fn freeze_spectrum() {
    invoke("freeze_spectrum", to_value(&()).unwrap()).await;
}

#[derive(Serialize, Deserialize)]
struct IdArgs {
    id: usize
}

pub async fn delete_frozen_spectrum(id: usize) {
    invoke("delete_frozen_spectrum", to_value(&IdArgs { id } ).unwrap()).await;
}

pub async fn get_frozen_spectrum_path(id: usize) -> String {
    let from_back = invoke("get_frozen_spectrum_path", to_value(&IdArgs { id }).unwrap()).await;
    let obj_rebuilt: String = from_value(from_back).unwrap();

    obj_rebuilt
}

pub async fn save_frozen_spectrum(id: usize) {
    invoke("save_frozen_spectrum", to_value(&IdArgs { id } ).unwrap()).await;
}

#[derive(Serialize, Deserialize)]
struct SaveContinuousArgs {
    save: bool
}

pub async fn save_continuous(save: bool) {
    invoke("save_continuous", to_value(&SaveContinuousArgs { save } ).unwrap()).await;
}

pub async fn get_saving() -> bool {
    let from_back = invoke("get_saving", to_value(&()).unwrap()).await;
    let obj_rebuilt: bool = from_value(from_back).unwrap();

    obj_rebuilt
}

#[derive(Serialize, Deserialize, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Connected,
    Reading
}

pub async fn get_connection_state() -> Option<ConnectionState> {
    let from_back = invoke("get_connection_state", to_value(&()).unwrap()).await;
    let obj_rebuilt: Option<ConnectionState> = from_value(from_back).unwrap();

    obj_rebuilt
}

pub async fn connect_acquisitor() {
    invoke("connect_acquisitor", to_value(&()).unwrap()).await;
}

pub async fn disconnect_acquisitor() {
    invoke("disconnect_acquisitor", to_value(&()).unwrap()).await;
}

pub async fn acquisitor_start_reading() {
    invoke("acquisitor_start_reading", to_value(&()).unwrap()).await;
}

pub async fn acquisitor_stop_reading() {
    invoke("acquisitor_stop_reading", to_value(&()).unwrap()).await;
}

pub async fn update_backend_config() {
    invoke("update_backend_config", to_value(&()).unwrap()).await;
}

pub async fn pick_folder() -> Option<PathBuf> {
    let from_back = invoke("pick_folder", to_value(&()).unwrap()).await;
    let obj_rebuilt: Option<PathBuf> = from_value(from_back).unwrap();

    obj_rebuilt
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileReaderConfig {
    pub auto_save_path: PathBuf,
    pub watcher_path: PathBuf,
    pub wavelength_limits: Option<(f64, f64)>,
    pub power_limits: Option<(f64, f64)>,
}

pub fn empty_back_config() -> FileReaderConfig {
    FileReaderConfig {
        auto_save_path: PathBuf::new(),
        watcher_path: PathBuf::new(),
        wavelength_limits: None,
        power_limits: None
    }
}

pub async fn get_back_config() -> Option<FileReaderConfig> {
    let from_back = invoke("get_back_config", to_value(&()).unwrap()).await;
    let obj_rebuilt: Option<FileReaderConfig> = from_value(from_back).unwrap();

    obj_rebuilt
}
