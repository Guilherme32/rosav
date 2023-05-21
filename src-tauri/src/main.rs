#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use app::*;
// use std::thread::sleep;
// use std::time::Duration;
use serde::{Serialize, Deserialize};
use std::sync::{ atomic, Mutex, mpsc };
use chrono::prelude::*;
use file_reader::ReaderState;

use tauri::api::dialog::FileDialogBuilder;


#[tauri::command]
fn hello() {
    println!("Hello");
}

#[tauri::command]
fn print_backend(msg: &str) {
    println!("From front: {}", msg);
}

#[tauri::command]
fn unread_spectrum(reader: tauri::State<file_reader::FileReader>) -> bool {
    reader.unread_spectrum.load(atomic::Ordering::Relaxed)
}

#[tauri::command]
fn get_last_spectrum_path(
    reader: tauri::State<file_reader::FileReader>,
    window: tauri::Window
) -> String 
{
    reader.update_limits();
    reader.get_last_spectrum_path(get_svg_size(window)).unwrap_or(String::new())
}

#[tauri::command]
fn get_window_size(window: tauri::Window) -> (u32, u32) {
    let win_size = window.inner_size().expect("Could not get window size");
    let scale = window.scale_factor().expect("Could not get window scale");

    (((win_size.width as f64) / scale).round() as u32, 
     ((win_size.height as f64) / scale).round() as u32)
}

#[tauri::command]
fn get_svg_size(window: tauri::Window) -> (u32, u32) {
    let win_size = window.inner_size().expect("Could not get window size");
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
fn get_last_logs(logs: tauri::State<Mutex<mpsc::Receiver<Log>>>) -> Vec::<Log> {
    let logs = logs.lock().unwrap();
    logs.try_recv().into_iter().collect()
}

#[tauri::command]
fn get_time() -> String {
    Local::now().format("(%H:%M)").to_string()
}

#[tauri::command]
fn get_wavelength_limits(reader: tauri::State<file_reader::FileReader>) -> (f64, f64) {
    let limits = match reader.spectrum_limits.lock() {
        Ok(limits) => limits,
        Err(_) => {
            println!("[MWL] Could not get the lock to read the limits");
            return (1000.0, 2000.0);
        }
    };

    if let Some(limits) = &*limits {
        limits.wavelength
    } else {
        (1010.0, 1990.0)
    }
}

#[tauri::command]
fn get_power_limits(reader: tauri::State<file_reader::FileReader>) -> (f64, f64) {
    let limits = match reader.spectrum_limits.lock() {
        Ok(limits) => limits,
        Err(_) => {
            println!("[MWL] Could not get the lock to read the limits");
            return (10.0, -5.0);
        }
    };

    if let Some(limits) = &*limits {
        (limits.power.1, limits.power.0)
    } else {
        (10.5, -10.0)
    }
}

#[tauri::command]
fn freeze_spectrum(reader: tauri::State<file_reader::FileReader>) {
    reader.freeze_spectrum();
}

#[tauri::command]
fn delete_frozen_spectrum(id: usize, reader: tauri::State<file_reader::FileReader>) {
    reader.delete_frozen_spectrum(id);
}

#[tauri::command]
fn get_frozen_spectrum_path(
    id: usize,
    reader: tauri::State<file_reader::FileReader>,
    window: tauri::Window
) -> String {
    reader.get_frozen_spectrum_path(id, get_svg_size(window))
        .unwrap_or(String::new())
}

#[tauri::command]
fn save_frozen_spectrum(
    id: usize,
    reader: tauri::State<file_reader::FileReader>,
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
                        Ok(_) => log_info(&log_tx, format!("[MSF] Spectrum {} saved", id)),
                        Err(error) => log_error(&log_tx, 
                            format!("[MSF] Failed to save spectrum {} ({})", id, error))
                    };
                }
            });
    }
}

#[tauri::command]
fn save_continuous(save: bool, reader:tauri::State<file_reader::FileReader>) {
    reader.saving_new.store(save, atomic::Ordering::Relaxed);
}

#[tauri::command]
fn get_saving(reader: tauri::State<file_reader::FileReader>) -> bool {
    reader.saving_new.load(atomic::Ordering::Relaxed)
}


#[derive(Serialize, Deserialize)]
enum ConnectionState {
    Disconnected,
    Connected,
    Reading
}

#[tauri::command]
fn get_connection_state(reader: tauri::State<file_reader::FileReader>) -> Option<ConnectionState> {
    let state = match reader.state.lock() {
        Ok(state) => state,
        Err(_) => {
            reader.log_error("[MCN] Failed to acquire state lock".to_string());
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
fn connect_acquisitor(reader: tauri::State<file_reader::FileReader>) {
    match reader.connect() {
        _ => ()
    }
}

#[tauri::command]
fn disconnect_acquisitor(reader: tauri::State<file_reader::FileReader>) {
    match reader.disconnect() {
        _ => ()
    }
}

#[tauri::command]
fn acquisitor_start_reading(reader: tauri::State<file_reader::FileReader>) {
    match reader.start_reading() {
        _ => ()
    }
}

#[tauri::command]
fn acquisitor_stop_reading(reader: tauri::State<file_reader::FileReader>) {
    match reader.stop_reading() {
        _ => ()
    }
}


fn main() {
    let (log_tx, log_rx) = mpsc::sync_channel::<Log>(64);
    log_info(&log_tx, "[MST] Starting the program".to_string());

    let reader = file_reader::new_file_reader("D:\\test".to_string(), log_tx);

    tauri::Builder::default()
        .manage(reader)
        .manage(Mutex::new(log_rx))
        .invoke_handler(tauri::generate_handler![
            hello,
            print_backend,
            unread_spectrum,
            get_last_spectrum_path,
            get_window_size,
            get_svg_size,
            get_last_logs,
            get_wavelength_limits,
            get_power_limits,
            get_time,
            freeze_spectrum,
            delete_frozen_spectrum,
            get_frozen_spectrum_path,
            save_frozen_spectrum,
            save_continuous,
            get_saving,
            get_connection_state,
            connect_acquisitor,
            disconnect_acquisitor,
            acquisitor_start_reading,
            acquisitor_stop_reading,
        ]).run(tauri::generate_context!())
        .expect("error while running tauri application");
}
