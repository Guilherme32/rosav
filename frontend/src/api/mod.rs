#![allow(non_snake_case)] // Tauri communication requires camelCase

use serde::{Deserialize, Serialize};
use serde_wasm_bindgen::{from_value, to_value};
use std::fmt;
use wasm_bindgen::prelude::*;

use std::path::PathBuf;

pub mod acquisitors;
use acquisitors::*;

// TODO fix the order to match the one on the backend

// Region: Returned structs definition

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub struct Log {
    pub msg: String,
    pub log_type: LogType,
}

#[derive(Serialize, Deserialize, Clone, PartialEq)]
pub enum LogType {
    Info,
    Warning,
    Error,
}
impl fmt::Display for LogType {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        match self {
            LogType::Info => write!(f, "info"),
            LogType::Warning => write!(f, "warning"),
            LogType::Error => write!(f, "error"),
        }
    }
}

#[derive(Serialize, Deserialize, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Connected,
    Reading,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(tag = "type")]
pub enum CriticalDetection {
    None,
    Simple { prominence: f64 },
    Lorentz { prominence: f64 },
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct HandlerConfig {
    pub auto_save_path: PathBuf,
    pub wavelength_limits: Option<(f64, f64)>,
    pub power_limits: Option<(f64, f64)>,
    pub acquisitor: acquisitors::AcquisitorSimple,
    pub valley_detection: CriticalDetection,
    pub peak_detection: CriticalDetection,
    pub shadow_length: usize,
}

pub fn empty_handler_config() -> HandlerConfig {
    HandlerConfig {
        auto_save_path: PathBuf::new(),
        wavelength_limits: None,
        power_limits: None,
        acquisitor: AcquisitorSimple::FileReader,
        valley_detection: CriticalDetection::None,
        peak_detection: CriticalDetection::None,
        shadow_length: 0,
    }
}

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
    msg: &'a str,
}

pub async fn print_backend(msg: &str) {
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

pub async fn get_last_spectrum_valleys_points() -> Vec<(f64, f64)> {
    let from_back = invoke("get_last_spectrum_valleys_points", to_value(&()).unwrap()).await;
    let obj_rebuilt: Vec<(f64, f64)> = from_value(from_back).unwrap();

    obj_rebuilt
}

pub async fn get_last_spectrum_peaks_points() -> Vec<(f64, f64)> {
    let from_back = invoke("get_last_spectrum_peaks_points", to_value(&()).unwrap()).await;
    let obj_rebuilt: Vec<(f64, f64)> = from_value(from_back).unwrap();

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

pub async fn get_last_logs() -> Vec<Log> {
    let from_back = invoke("get_last_logs", to_value(&()).unwrap()).await;
    let obj_rebuilt: Vec<Log> = from_value(from_back).unwrap();

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

pub async fn get_valley_detection() -> CriticalDetection {
    let from_back = invoke("get_valley_detection", to_value(&()).unwrap()).await;
    let obj_rebuilt: CriticalDetection = from_value(from_back).unwrap();

    obj_rebuilt
}

pub async fn get_peak_detection() -> CriticalDetection {
    let from_back = invoke("get_peak_detection", to_value(&()).unwrap()).await;
    let obj_rebuilt: CriticalDetection = from_value(from_back).unwrap();

    obj_rebuilt
}

pub async fn freeze_spectrum() {
    invoke("freeze_spectrum", to_value(&()).unwrap()).await;
}

#[derive(Serialize, Deserialize)]
struct IdArgs {
    id: usize,
}

pub async fn delete_frozen_spectrum(id: usize) {
    invoke("delete_frozen_spectrum", to_value(&IdArgs { id }).unwrap()).await;
}

pub async fn get_frozen_spectrum_path(id: usize) -> String {
    let from_back = invoke(
        "get_frozen_spectrum_path",
        to_value(&IdArgs { id }).unwrap(),
    )
    .await;
    let obj_rebuilt: String = from_value(from_back).unwrap();

    obj_rebuilt
}

pub async fn get_frozen_spectrum_valleys_points(id: usize) -> Vec<(f64, f64)> {
    let from_back = invoke(
        "get_frozen_spectrum_valleys_points",
        to_value(&IdArgs { id }).unwrap(),
    )
    .await;
    let obj_rebuilt: Vec<(f64, f64)> = from_value(from_back).unwrap();

    obj_rebuilt
}

pub async fn get_frozen_spectrum_peaks_points(id: usize) -> Vec<(f64, f64)> {
    let from_back = invoke(
        "get_frozen_spectrum_peaks_points",
        to_value(&IdArgs { id }).unwrap(),
    )
    .await;
    let obj_rebuilt: Vec<(f64, f64)> = from_value(from_back).unwrap();

    obj_rebuilt
}

pub async fn pick_folder() -> Option<PathBuf> {
    let from_back = invoke("pick_folder", to_value(&()).unwrap()).await;
    let obj_rebuilt: Option<PathBuf> = from_value(from_back).unwrap();

    obj_rebuilt
}

pub async fn save_frozen_spectrum(id: usize) {
    invoke("save_frozen_spectrum", to_value(&IdArgs { id }).unwrap()).await;
}

pub async fn save_all_spectra() {
    invoke("save_all_spectra", to_value(&()).unwrap()).await;
}

pub async fn get_shadow_paths() -> Vec<String> {
    let from_back = invoke("get_shadow_paths", to_value(&()).unwrap()).await;
    let obj_rebuilt: Vec<String> = from_value(from_back).unwrap();

    obj_rebuilt
}

#[derive(Serialize, Deserialize)]
struct SaveContinuousArgs {
    save: bool,
}

pub async fn save_continuous(save: bool) {
    invoke(
        "save_continuous",
        to_value(&SaveContinuousArgs { save }).unwrap(),
    )
    .await;
}

pub async fn get_saving() -> bool {
    let from_back = invoke("get_saving", to_value(&()).unwrap()).await;
    let obj_rebuilt: bool = from_value(from_back).unwrap();

    obj_rebuilt
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

pub async fn acquisitor_read_single() {
    invoke("acquisitor_read_single", to_value(&()).unwrap()).await;
}

pub async fn acquisitor_stop_reading() {
    invoke("acquisitor_stop_reading", to_value(&()).unwrap()).await;
}

// pub async fn pick_folder() -> Option<PathBuf> {
//     let from_back = invoke("pick_folder", to_value(&()).unwrap()).await;
//     let obj_rebuilt: Option<PathBuf> = from_value(from_back).unwrap();

//     obj_rebuilt
// }

pub async fn get_handler_config() -> HandlerConfig {
    let from_back = invoke("get_handler_config", to_value(&()).unwrap()).await;
    let obj_rebuilt: HandlerConfig = from_value(from_back).unwrap();

    obj_rebuilt
}

#[derive(Serialize, Deserialize)]
struct HandlerConfigArgs {
    newConfig: HandlerConfig, // The tauri communication requires camelCase
}

pub async fn apply_handler_config(newConfig: HandlerConfig) {
    invoke(
        "apply_handler_config",
        to_value(&HandlerConfigArgs { newConfig }).unwrap(),
    )
    .await;
}

pub async fn change_limits(wavelength: Option<(f64, f64)>, power: Option<(f64, f64)>) {
    let mut config = get_handler_config().await;

    config.wavelength_limits = wavelength;
    config.power_limits = power;

    apply_handler_config(config).await;
}

// SubRegion: Acquisitor config --------------------------------------------------------------------

pub async fn get_acquisitor_config() -> AcquisitorConfig {
    let from_back = invoke("get_acquisitor_config", to_value(&()).unwrap()).await;
    let obj_rebuilt: AcquisitorConfig = from_value(from_back).unwrap();

    obj_rebuilt
}

#[derive(Serialize, Deserialize)]
struct AcquisitorConfigArgs {
    newConfig: AcquisitorConfig, // The tauri communication requires camelCase
}

pub async fn apply_acquisitor_config(newConfig: AcquisitorConfig) {
    invoke(
        "apply_acquisitor_config",
        to_value(&AcquisitorConfigArgs { newConfig }).unwrap(),
    )
    .await;
}
