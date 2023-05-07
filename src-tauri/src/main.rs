#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use app::file_reader;

fn main() {
    file_reader::test();

    let reader = file_reader::new_file_reader("D:\\test".to_string());
    let reader = reader.connect().unwrap();
    let _reader = reader.read_continuous();

    tauri::Builder::default()
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
