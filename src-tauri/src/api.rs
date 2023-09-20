use chrono::prelude::*;
use std::fs::create_dir;
use std::path::PathBuf;
use std::sync::{atomic, mpsc, Mutex};
use tauri::api::dialog::{blocking, FileDialogBuilder};

use crate::spectrum_handler::time_series::TimeSeriesGroupPaths;
use crate::spectrum_handler::TimeSeriesConfig;
use crate::*;
use spectrum::CriticalDetection;
use spectrum_handler::{AcquisitorConfig, HandlerConfig, SpectrumHandler, State as HandlerState};

use config::{write_acquisitor_config, write_handler_config};

// TODO change all commands to async

// Region: basic functions -----------------------------------------------------

#[tauri::command]
pub fn hello() {
    println!("Hello");
}

#[tauri::command]
pub fn print_backend(msg: &str) {
    println!("From front: {}", msg);
}

#[tauri::command]
pub fn get_window_size(window: tauri::Window) -> (u32, u32) {
    let win_size = match window.inner_size() {
        Ok(size) => size,
        Err(_) => return (0, 0),
    };
    let scale = match window.scale_factor() {
        Ok(scale) => scale,
        Err(_) => return (0, 0),
    };

    let compensation = if cfg!(windows) {
        (0, 0)
    } else if cfg!(unix) {
        // The webkit on my Fedora38 is giving nonsensical values
        (50, 90)
    } else {
        (0, 0)
    };

    (
        ((win_size.width - compensation.0) as f64 / scale).round() as u32,
        ((win_size.height - compensation.1) as f64 / scale).round() as u32,
    )
}

#[tauri::command]
pub fn get_last_logs(logs: tauri::State<Mutex<mpsc::Receiver<Log>>>) -> Vec<Log> {
    let logs = logs.lock().unwrap();
    logs.try_recv().into_iter().collect()
}

#[tauri::command]
pub fn get_time() -> String {
    Local::now().format("(%H:%M)").to_string()
}

#[tauri::command]
pub fn get_valley_detection(handler: tauri::State<SpectrumHandler>) -> CriticalDetection {
    handler.get_valley_detection()
}

#[tauri::command]
pub fn get_peak_detection(handler: tauri::State<SpectrumHandler>) -> CriticalDetection {
    handler.get_peak_detection()
}

#[tauri::command]
pub fn get_time_series_config(handler: tauri::State<SpectrumHandler>) -> TimeSeriesConfig {
    handler.get_time_series_config()
}

// Region: Graph / Plot / Spectrum related -------------------------------------
// SubRegion: Basic graph functions --------------------------------------------

#[tauri::command]
pub fn get_svg_size(window: tauri::Window) -> (u32, u32) {
    let win_size = get_window_size(window);

    if win_size.0 == 0 {
        // if minimized
        return (0, 0);
    }

    (win_size.0 - 23 - 200, win_size.1 - 27 - 32)
}

#[tauri::command]
pub fn get_wavelength_limits(handler: tauri::State<SpectrumHandler>) -> (f64, f64) {
    let limits = handler.get_limits(handler.get_max_power());

    if let Some(limits) = limits {
        limits.wavelength
    } else {
        (1010.0, 1990.0)
    }
}

#[tauri::command]
pub fn get_power_limits(handler: tauri::State<SpectrumHandler>) -> (f64, f64) {
    let normalize = true; // TODO send to config
    let offset = if normalize {
        let max_power = handler.get_max_power();
        if max_power == f64::NEG_INFINITY {
            0.0
        } else {
            -max_power
        }
    } else {
        0.0
    };

    let limits = handler.get_limits(handler.get_max_power());

    if let Some(limits) = limits {
        (limits.power.1 + offset, limits.power.0 + offset)
    } else {
        (10.5, -10.0)
    }
}

// Used for normalization at 0 dB
#[tauri::command]
pub fn get_max_power(handler: tauri::State<SpectrumHandler>) -> Option<f64> {
    let max_power = handler.get_max_power();

    if max_power == f64::NEG_INFINITY {
        None
    } else {
        Some(max_power)
    }
}

// SubRegion: Last spectrum functions ------------------------------------------

#[tauri::command]
pub fn unread_spectrum(handler: tauri::State<SpectrumHandler>) -> bool {
    handler.unread_spectrum.load(atomic::Ordering::Relaxed)
}

#[tauri::command]
pub fn get_last_spectrum_path(
    handler: tauri::State<SpectrumHandler>,
    window: tauri::Window,
) -> String {
    handler.update_limits();
    handler
        .get_last_spectrum_path(get_svg_size(window))
        .unwrap_or(String::new())
}

#[tauri::command]
pub async fn get_last_spectrum_valleys_points(
    handler: tauri::State<'_, SpectrumHandler>,
    window: tauri::Window,
) -> Result<Vec<(f64, f64)>, ()> {
    let points = handler.get_last_spectrum_valleys_points(get_svg_size(window));
    Ok(points.unwrap_or(vec![]))
}

#[tauri::command]
pub async fn get_last_spectrum_peaks_points(
    handler: tauri::State<'_, SpectrumHandler>,
    window: tauri::Window,
) -> Result<Vec<(f64, f64)>, ()> {
    let points = handler.get_last_spectrum_peaks_points(get_svg_size(window));
    Ok(points.unwrap_or(vec![]))
}

#[tauri::command]
pub fn save_continuous(save: bool, handler: tauri::State<SpectrumHandler>) {
    handler.saving_new.store(save, atomic::Ordering::Relaxed);
}

#[tauri::command]
pub fn get_saving(handler: tauri::State<SpectrumHandler>) -> bool {
    handler.saving_new.load(atomic::Ordering::Relaxed)
}

// SubRegion: Frozen spectra functions -----------------------------------------

#[tauri::command]
pub fn freeze_spectrum(handler: tauri::State<SpectrumHandler>) {
    handler.freeze_spectrum();
}

#[tauri::command]
pub fn delete_frozen_spectrum(id: usize, handler: tauri::State<SpectrumHandler>) {
    handler.delete_frozen_spectrum(id);
    handler.update_limits();
}

#[tauri::command]
pub fn get_frozen_spectrum_path(
    id: usize,
    handler: tauri::State<SpectrumHandler>,
    window: tauri::Window,
) -> String {
    handler
        .get_frozen_spectrum_path(id, get_svg_size(window))
        .unwrap_or(String::new())
}

#[tauri::command]
pub async fn get_frozen_spectrum_valleys_points(
    id: usize,
    handler: tauri::State<'_, SpectrumHandler>,
    window: tauri::Window,
) -> Result<Vec<(f64, f64)>, ()> {
    let points = handler.get_frozen_spectrum_valleys_points(id, get_svg_size(window));
    Ok(points.unwrap_or(vec![]))
}

#[tauri::command]
pub async fn get_frozen_spectrum_peaks_points(
    id: usize,
    handler: tauri::State<'_, SpectrumHandler>,
    window: tauri::Window,
) -> Result<Vec<(f64, f64)>, ()> {
    let points = handler.get_frozen_spectrum_peaks_points(id, get_svg_size(window));
    Ok(points.unwrap_or(vec![]))
}

#[tauri::command]
pub fn save_frozen_spectrum(
    id: usize,
    handler: tauri::State<SpectrumHandler>,
    window: tauri::Window,
) {
    let spectrum = handler.clone_frozen(id);
    if let Some(spectrum) = spectrum {
        let log_tx = handler.log_sender.clone();

        FileDialogBuilder::new()
            .add_filter("text", &["txt"])
            .set_file_name("spectrum")
            .set_parent(&window)
            .save_file(move |path| {
                if let Some(path) = path {
                    return match spectrum.save(&path) {
                        Ok(_) => log_info(&log_tx, format!("[ASF] Espectro {} salvo", id)),
                        Err(error) => log_error(
                            &log_tx,
                            format!("[ASF] falha ao salvar espectro {} ({})", id, error),
                        ),
                    };
                }
            });
    }
}

#[tauri::command]
pub async fn save_all_spectra(
    handler: tauri::State<'_, SpectrumHandler>,
    window: tauri::Window,
) -> Result<(), ()> {
    if handler.get_frozen_length() == 0 {
        return Ok(());
    }

    let folder_path = blocking::FileDialogBuilder::new()
        .set_parent(&window)
        .pick_folder();

    if let Some(folder_path) = folder_path {
        let time = Local::now().format("%Y%m%d-%H%M%S").to_string();
        let folder_path = folder_path.join(time);
        let folder_path_copy = folder_path.clone();
        if let Err(error) = create_dir(folder_path_copy) {
            handler.log_error(format!(
                "[ASA] Não foi possível salvar os espectros ({})",
                error
            ));
            return Err(());
        }

        for id in 0..handler.get_frozen_length() {
            let file_path = folder_path.join(format!("spectrum{:03}.txt", id));
            handler.save_frozen(id, &file_path);
        }

        let info_file_path = folder_path.join("spectra_info.json");
        if let Err(error) = handler.save_all_frozen_info(&info_file_path) {
            handler.log_error(format!(
                "[ASA] Não foi possível salvar as informações dos espectros ({})",
                error
            ));
            return Err(());
        } else {
            handler.log_info("[ASA] Info dos espectros salva".to_string());
        }
    }

    Ok(())
}

// SubRegion: Shadow stuff -----------------------------------------------------

#[tauri::command]
pub fn get_shadow_paths(
    handler: tauri::State<SpectrumHandler>,
    window: tauri::Window,
) -> Vec<String> {
    handler.get_shadow_paths(get_svg_size(window))
}

// SubRegion: Time Series stuff -----------------------------------------------------

#[tauri::command]
pub fn get_time_series_paths(
    handler: tauri::State<SpectrumHandler>,
    window: tauri::Window,
) -> TimeSeriesGroupPaths {
    handler.get_time_series_paths(get_svg_size(window))
}

// Region: Acquisitor functions ------------------------------------------------

#[tauri::command]
pub fn get_connection_state(handler: tauri::State<SpectrumHandler>) -> HandlerState {
    handler.get_state()
}

#[tauri::command]
pub fn connect_acquisitor(handler: tauri::State<SpectrumHandler>) {
    let _result = handler.connect();
}

#[tauri::command]
pub fn disconnect_acquisitor(handler: tauri::State<SpectrumHandler>) {
    let _result = handler.disconnect();
}

#[tauri::command]
pub fn acquisitor_start_reading(handler: tauri::State<SpectrumHandler>) {
    let _result = handler.start_reading();
}

#[tauri::command]
pub fn acquisitor_read_single(handler: tauri::State<SpectrumHandler>) {
    let _result = handler.read_single();
}

#[tauri::command]
pub fn acquisitor_stop_reading(handler: tauri::State<SpectrumHandler>) {
    let _result = handler.stop_reading();
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
    if let Err(error) = write_handler_config(&new_config) {
        // write to file
        handler.log_error(format!(
            "[AAB] Não consegui escrever no arquivo de \
            config. ({})",
            error
        ));
    };

    handler.update_config(new_config);
}

//SubRegion: Acquisitor config -------------------------------------------------

#[tauri::command]
pub fn get_acquisitor_config(handler: tauri::State<SpectrumHandler>) -> AcquisitorConfig {
    handler.get_acquisitor_config()
}

#[tauri::command]
pub fn apply_acquisitor_config(
    new_config: AcquisitorConfig,
    handler: tauri::State<SpectrumHandler>,
) {
    if let Err(error) = write_acquisitor_config(&new_config) {
        // write to file
        handler.log_error(format!(
            "[AAB] Não consegui escrever no arquivo de \
            config. ({})",
            error
        ));
    };

    handler.update_acquisitor_config(new_config);
}
