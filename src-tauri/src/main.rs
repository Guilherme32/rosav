#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use app::{ file_reader, Log, LogType, new_log };
// use std::thread::sleep;
// use std::time::Duration;
// use serde::{Serialize, Deserialize};
use std::sync::{ atomic, Mutex, mpsc };
use chrono::prelude::*;


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

// #[tauri::command]
// fn get_last_logs(logs: tauri::State<Mutex<Vec::<Log>>>) -> Vec::<Log> {
//     let mut logs_lock = logs.lock().unwrap();
//     let mut new_vec = Vec::<Log>::with_capacity((*logs_lock).len());
//     while !(*logs_lock).is_empty() {
//         new_vec.push((*logs_lock).remove(0));
//     }

//     new_vec
// }

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

fn main() {
    file_reader::test();

    let (log_tx, log_rx) = mpsc::sync_channel::<Log>(64);

    let mut reader = file_reader::new_file_reader("D:\\test".to_string(), log_tx);
    reader.connect().unwrap();
    reader.read_continuous().unwrap();

    let log = Mutex::new(Vec::<Log>::new());
    {
        let mut lock = log.lock().unwrap();
        (*lock).push(new_log("[STR] Started the program".to_string(), LogType::Info));
    };

    tauri::Builder::default()
        .manage(reader)
        .manage(log)
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
            get_frozen_spectrum_path
        ]).run(tauri::generate_context!())
        .expect("error while running tauri application");
}
