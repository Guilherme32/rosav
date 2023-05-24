#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

// use std::thread::sleep;
// use std::time::Duration;
// use serde::{Serialize, Deserialize};
use std::sync::{ Mutex, mpsc };
// use chrono::prelude::*;
// use tauri::api::dialog::FileDialogBuilder;

use app::*;
use app::api::*;


fn main() {
    let (log_tx, log_rx) = mpsc::sync_channel::<Log>(64);
    log_info(&log_tx, "[MST] Iniciando o programa".to_string());

    let config = if config_path().exists() {
        match get_config() {
            Ok(config) => config,
            Err(error) => {
                log_war(&log_tx, format!("[MST] Não foi possível ler a config. \
                    Usando a padrão. Erro: {}", error));
                default_config()
            } 
        }
    } else {
        let config = default_config();
        if let Err(error) = write_config(&config) {
            log_error(&log_tx, format!("[MST] Não consegui criar o arquivo de \
                config. ({})", error));
        };
        config
    };

    let reader = file_reader::new_file_reader(config, log_tx);

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
            update_backend_config,
            get_path
        ]).run(tauri::generate_context!())
        .expect("error while running tauri application");
}
