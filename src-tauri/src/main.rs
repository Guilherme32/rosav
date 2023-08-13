#![cfg_attr(
    all(not(debug_assertions), target_os = "windows"),
    windows_subsystem = "windows"
)]

use std::sync::{mpsc, Mutex};

use api::*;
use app::*;
use config::*;
use spectrum_handler::new_spectrum_handler;

fn main() {
    let (log_tx, log_rx) = mpsc::sync_channel::<Log>(64);
    log_info(&log_tx, "[MST] Iniciando o programa".to_string());

    let handler_config = match load_handler_config() {
        Ok(config) => config,
        Err(error) => {
            log_war(
                &log_tx,
                format!(
                    "[MST] Não foi possível ler a config. \
                    Usando a padrão. Erro: {}",
                    error
                ),
            );
            spectrum_handler::default_config()
        }
    };
    let handler = new_spectrum_handler(handler_config, log_tx);

    tauri::Builder::default()
        .manage(handler)
        .manage(Mutex::new(log_rx))
        .invoke_handler(tauri::generate_handler![
            hello,
            print_backend,
            unread_spectrum,
            get_last_spectrum_path,
            get_last_spectrum_valleys_points,
            get_window_size,
            get_svg_size,
            get_last_logs,
            get_wavelength_limits,
            get_power_limits,
            get_time,
            get_valley_detection,
            freeze_spectrum,
            delete_frozen_spectrum,
            get_frozen_spectrum_path,
            get_frozen_spectrum_valleys_points,
            save_frozen_spectrum,
            save_all_spectra,
            save_continuous,
            get_saving,
            get_connection_state,
            connect_acquisitor,
            disconnect_acquisitor,
            acquisitor_start_reading,
            acquisitor_stop_reading,
            pick_folder,
            get_handler_config,
            apply_handler_config,
            get_acquisitor_config,
            apply_acquisitor_config,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
