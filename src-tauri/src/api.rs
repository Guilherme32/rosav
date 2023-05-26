use std::sync::{ atomic, Mutex, mpsc };
use chrono::prelude::*;
use tauri::api::dialog::{ FileDialogBuilder, blocking };
use std::path::PathBuf;

use crate::*;
use spectrum_handler::{
    State as HandlerState,
    SpectrumHandler,
    HandlerConfig,
    AcquisitorConfig
};

use config::{ write_handler_config, write_acquisitor_config };


#[tauri::command]
pub fn hello() {
    println!("Hello");
}

#[tauri::command]
pub fn print_backend(msg: &str) {
    println!("From front: {}", msg);
}

#[tauri::command]
pub fn unread_spectrum(handler: tauri::State<SpectrumHandler>) -> bool {
    handler.unread_spectrum.load(atomic::Ordering::Relaxed)
}

#[tauri::command]
pub fn get_last_spectrum_path(
    handler: tauri::State<SpectrumHandler>,
    window: tauri::Window
) -> String 
{
    handler.update_limits();
    handler.get_last_spectrum_path(get_svg_size(window)).unwrap_or(String::new())
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
pub fn get_wavelength_limits(handler: tauri::State<SpectrumHandler>) -> (f64, f64) {
    let limits = handler.get_limits();

    if let Some(limits) = limits {
        limits.wavelength
    } else {
        (1010.0, 1990.0)
    }
}

#[tauri::command]
pub fn get_power_limits(handler: tauri::State<SpectrumHandler>) -> (f64, f64) {
    let limits = handler.get_limits();

    if let Some(limits) = limits {
        (limits.power.1, limits.power.0)
    } else {
        (10.5, -10.0)
    }
}

#[tauri::command]
pub fn freeze_spectrum(handler: tauri::State<SpectrumHandler>) {
    handler.freeze_spectrum();
}

#[tauri::command]
pub fn delete_frozen_spectrum(id: usize, handler: tauri::State<SpectrumHandler>) {
    handler.delete_frozen_spectrum(id);
}

#[tauri::command]
pub fn get_frozen_spectrum_path(
    id: usize,
    handler: tauri::State<SpectrumHandler>,
    window: tauri::Window
) -> String {
    handler.get_frozen_spectrum_path(id, get_svg_size(window))
        .unwrap_or(String::new())
}

#[tauri::command]
pub fn save_frozen_spectrum(
    id: usize,
    handler: tauri::State<SpectrumHandler>,
    window: tauri::Window
) {
    let spectrum = handler.clone_frozen(id);
    if let Some(spectrum) = spectrum {
        let log_tx = handler.log_sender.clone();

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
pub fn save_continuous(save: bool, handler:tauri::State<SpectrumHandler>) {
    handler.saving_new.store(save, atomic::Ordering::Relaxed);
}

#[tauri::command]
pub fn get_saving(handler: tauri::State<SpectrumHandler>) -> bool {
    handler.saving_new.load(atomic::Ordering::Relaxed)
}

#[tauri::command]
pub fn get_connection_state(handler: tauri::State<SpectrumHandler>) -> HandlerState {
    handler.get_state()
}

#[tauri::command]
pub fn connect_acquisitor(handler: tauri::State<SpectrumHandler>) {
    match handler.connect() {
        _ => ()
    }
}

#[tauri::command]
pub fn disconnect_acquisitor(handler: tauri::State<SpectrumHandler>) {
    match handler.disconnect() {
        _ => ()
    }
}

#[tauri::command]
pub fn acquisitor_start_reading(handler: tauri::State<SpectrumHandler>) {
    match handler.start_reading() {
        _ => ()
    }
}

#[tauri::command]
pub fn acquisitor_stop_reading(handler: tauri::State<SpectrumHandler>) {
    match handler.stop_reading() {
        _ => ()
    }
}

#[tauri::command]
pub async fn pick_folder(window: tauri::Window) -> Option<PathBuf> {
    blocking::FileDialogBuilder::new()
        .set_parent(&window)
        .pick_folder()
}


// Region: Config --------------------------------------------------------------
//SubRegion: Handler config ----------------------------------------------------

#[tauri::command]
pub fn get_handler_config(handler: tauri::State<SpectrumHandler>) -> HandlerConfig {
    handler.get_config()
}

#[tauri::command]
pub fn apply_handler_config(new_config: HandlerConfig, handler: tauri::State<SpectrumHandler>) {
    if let Err(error) = write_handler_config(&new_config) {                            // write to file
        handler.log_error(format!("[AAB] Não consegui escrever no arquivo de \
            config. ({})", error));
    };

    handler.update_config(new_config);
}


//SubRegion: Acquisitor config -------------------------------------------------

#[tauri::command]
pub fn get_acquisitor_config(handler: tauri::State<SpectrumHandler>) -> AcquisitorConfig {
    handler.get_acquisitor_config()
}

#[tauri::command]
pub fn apply_acquisitor_config(new_config: AcquisitorConfig, handler: tauri::State<SpectrumHandler>) {
    if let Err(error) = write_acquisitor_config(&new_config) {                            // write to file
        handler.log_error(format!("[AAB] Não consegui escrever no arquivo de \
            config. ({})", error));
    };

    handler.update_acquisitor_config(new_config);
}
