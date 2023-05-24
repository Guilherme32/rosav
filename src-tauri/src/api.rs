use serde::{Serialize, Deserialize};
use std::sync::{ atomic, Mutex, mpsc };
use chrono::prelude::*;
use tauri::api::dialog::{ FileDialogBuilder, blocking };

use crate::*;
use file_reader::{ ReaderState, FileReader };

#[tauri::command]
pub fn hello() {
    println!("Hello");
}

#[tauri::command]
pub fn print_backend(msg: &str) {
    println!("From front: {}", msg);
}

#[tauri::command]
pub fn unread_spectrum(reader: tauri::State<FileReader>) -> bool {
    reader.unread_spectrum.load(atomic::Ordering::Relaxed)
}

#[tauri::command]
pub fn get_last_spectrum_path(
    reader: tauri::State<FileReader>,
    window: tauri::Window
) -> String 
{
    reader.update_limits();
    reader.get_last_spectrum_path(get_svg_size(window)).unwrap_or(String::new())
}

#[tauri::command]
pub fn get_window_size(window: tauri::Window) -> (u32, u32) {
    let win_size = window.inner_size().expect("Could not get window size");            // TODO lidar com os erros
    let scale = window.scale_factor().expect("Could not get window scale");

    (((win_size.width as f64) / scale).round() as u32, 
     ((win_size.height as f64) / scale).round() as u32)
}

#[tauri::command]
pub fn get_svg_size(window: tauri::Window) -> (u32, u32) {
    let win_size = window.inner_size().expect("Could not get window size");            // TODO lidar com os erros
    let scale = window.scale_factor().expect("Could not get window scale");
    let win_size_scaled = (((win_size.width as f64) / scale).round() as u32, 
                           ((win_size.height as f64) / scale).round() as u32);

 
    if win_size.width == 0 {            // if minimized
        return (0, 0);
    }

    (win_size_scaled.0 - 23 - 200,
     win_size_scaled.1 - 27 - 32)
}

#[tauri::command]
pub fn get_last_logs(logs: tauri::State<Mutex<mpsc::Receiver<Log>>>) -> Vec::<Log> {
    let logs = logs.lock().unwrap();
    logs.try_recv().into_iter().collect()
}

#[tauri::command]
pub fn get_time() -> String {
    Local::now().format("(%H:%M)").to_string()
}

#[tauri::command]
pub fn get_wavelength_limits(reader: tauri::State<FileReader>) -> (f64, f64) {
    let limits = reader.get_limits();

    if let Some(limits) = limits {
        limits.wavelength
    } else {
        (1010.0, 1990.0)
    }
}

#[tauri::command]
pub fn get_power_limits(reader: tauri::State<FileReader>) -> (f64, f64) {
    let limits = reader.get_limits();

    if let Some(limits) = limits {
        (limits.power.1, limits.power.0)
    } else {
        (10.5, -10.0)
    }
}

#[tauri::command]
pub fn freeze_spectrum(reader: tauri::State<FileReader>) {
    reader.freeze_spectrum();
}

#[tauri::command]
pub fn delete_frozen_spectrum(id: usize, reader: tauri::State<FileReader>) {
    reader.delete_frozen_spectrum(id);
}

#[tauri::command]
pub fn get_frozen_spectrum_path(
    id: usize,
    reader: tauri::State<FileReader>,
    window: tauri::Window
) -> String {
    reader.get_frozen_spectrum_path(id, get_svg_size(window))
        .unwrap_or(String::new())
}

#[tauri::command]
pub fn save_frozen_spectrum(
    id: usize,
    reader: tauri::State<FileReader>,
    window: tauri::Window
) {
    let spectrum = reader.clone_frozen(id);
    if let Some(spectrum) = spectrum {
        let log_tx = reader.log_sender.clone();

        FileDialogBuilder::new()
            .add_filter("text", &["txt", ])
            .set_file_name("spectrum")
            .set_parent(&window)
            .save_file(move |path| {
                if let Some(path) = path {
                    return match spectrum.save(&path) {
                        Ok(_) => log_info(&log_tx, format!("[MSF] Espectro {} salvo", id)),
                        Err(error) => log_error(&log_tx, 
                            format!("[MSF] falha ao salvar espectro {} ({})", id, error))
                    };
                }
            });
    }
}

#[tauri::command]
pub fn save_continuous(save: bool, reader:tauri::State<FileReader>) {
    reader.saving_new.store(save, atomic::Ordering::Relaxed);
}

#[tauri::command]
pub fn get_saving(reader: tauri::State<FileReader>) -> bool {
    reader.saving_new.load(atomic::Ordering::Relaxed)
}


#[derive(Serialize, Deserialize)]
pub enum ConnectionState {
    Disconnected,
    Connected,
    Reading
}

#[tauri::command]
pub fn get_connection_state(reader: tauri::State<FileReader>) -> Option<ConnectionState> {
    let state = match reader.state.lock() {
        Ok(state) => state,
        Err(_) => {
            reader.log_error("[MCN] Falha ao adquirir lock para 'state'".to_string());
            return None;
        }
    };

    match *state {
        ReaderState::Disconnected => Some(ConnectionState::Disconnected),
        ReaderState::Connected => Some(ConnectionState::Connected),
        ReaderState::Reading(_) => Some(ConnectionState::Reading)
    }
}

#[tauri::command]
pub fn connect_acquisitor(reader: tauri::State<FileReader>) {
    match reader.connect() {
        _ => ()
    }
}

#[tauri::command]
pub fn disconnect_acquisitor(reader: tauri::State<FileReader>) {
    match reader.disconnect() {
        _ => ()
    }
}

#[tauri::command]
pub fn acquisitor_start_reading(reader: tauri::State<FileReader>) {
    match reader.start_reading() {
        _ => ()
    }
}

#[tauri::command]
pub fn acquisitor_stop_reading(reader: tauri::State<FileReader>) {
    match reader.stop_reading() {
        _ => ()
    }
}

#[tauri::command]
pub fn update_backend_config(reader: tauri::State<FileReader>) {
    match get_config() {
        Ok(config) => reader.update_config(config),
        Err(error) => reader.log_error(format!("[MUC] Não foi possível \
            atualizar a config. ({})", error))
    }
}

#[tauri::command]
pub async fn get_path(window: tauri::Window) -> Option<PathBuf> {
    blocking::FileDialogBuilder::new()
        .add_filter("text", &["txt", ])
        .set_file_name("spectrum")
        .set_parent(&window)
        .save_file()
}
