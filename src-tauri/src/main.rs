#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use app::file_reader::{ self, Continuous };
// use std::thread::sleep;
// use std::time::Duration;
use serde::{Serialize, Deserialize};
use std::sync::{ atomic, Mutex };

#[derive(Serialize, Deserialize, Debug)]
struct Log {
    id: u32,
    msg: String,
    log_type: LogType
}

#[derive(Serialize, Deserialize, Debug)]
enum LogType {
    Info,
    Warning,
    Error
}

#[tauri::command]
fn unread_spectrum(reader: tauri::State<file_reader::FileReader<Continuous>>) -> bool {
    reader.unread_spectrum.load(atomic::Ordering::Relaxed)
}

#[tauri::command]
fn get_last_spectrum_path(reader: tauri::State<file_reader::FileReader<Continuous>>) -> Option<String> {
    reader.get_last_spectrum_path((480.0, 360.0))
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
fn get_last_logs(logs: tauri::State<Mutex<Vec::<Log>>>) -> Vec::<Log> {
    let mut logs_lock = logs.lock().unwrap();
    let mut new_vec = Vec::<Log>::with_capacity((*logs_lock).len());
    while !(*logs_lock).is_empty() {
        new_vec.push((*logs_lock).remove(0));
    }

    new_vec
}

fn main() {
    file_reader::test();

    let reader = file_reader::new_file_reader("D:\\test".to_string());
    let reader = reader.connect().unwrap();
    let reader = reader.read_continuous().unwrap();

    let log = Mutex::new(Vec::<Log>::new());
    {
        let mut lock = log.lock().unwrap();
        (*lock).push(Log {
            id: 0,
            msg: "[STR] Started the program".to_string(),
            log_type: LogType::Info
        });
    };
        
    // loop {
    //     sleep(Duration::from_secs(1));
    //     let unread = reader.unread_spectrum.load(atomic::Ordering::Relaxed);
    //     println!("-->{}", unread);
    //     if unread {
    //         if let Some(specpath) = reader.get_last_spectrum_path((200.0, 200.0)) {
    //             println!("{}", specpath);
    //         }
    //         break;
    //     }
    // }

    tauri::Builder::default()
        .manage(reader)
        .manage(log)
        .invoke_handler(tauri::generate_handler![
            unread_spectrum,
            get_last_spectrum_path,
            get_window_size,
            get_svg_size,
            get_last_logs
        ]).run(tauri::generate_context!())
        .expect("error while running tauri application");
}
