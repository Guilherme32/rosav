#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use app::file_reader::{ self, Continuous };
use std::thread::sleep;
use std::time::Duration;
use std::sync::atomic;


#[tauri::command]
fn unread_spectrum(reader: tauri::State<file_reader::FileReader<Continuous>>) -> bool {
    return reader.unread_spectrum.load(atomic::Ordering::Relaxed);
}

#[tauri::command]
fn get_last_spectrum_path(reader: tauri::State<file_reader::FileReader<Continuous>>) -> Option<String> {
    reader.get_last_spectrum_path((480.0, 360.0))
}

fn main() {
    file_reader::test();

    let reader = file_reader::new_file_reader("D:\\test".to_string());
    let reader = reader.connect().unwrap();
    let reader = reader.read_continuous().unwrap();

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
        .invoke_handler(tauri::generate_handler![unread_spectrum, get_last_spectrum_path])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
