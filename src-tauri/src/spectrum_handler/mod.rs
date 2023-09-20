use crate::{log_error, log_info, log_war, Log};

use std::error::Error;
use std::fs::File;
use std::io::prelude::*;
use std::path::Path;

use std::sync::atomic::{self, AtomicBool};
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex};

use std::path::PathBuf;

use chrono::Local;
use serde::{Deserialize, Serialize};

use crate::spectrum::*;

pub mod acquisitors;
pub mod time_series;

use time_series::TimeSeriesGroup;

use acquisitors::{
    file_reader::{FileReader, FileReaderConfig},
    ibsen_imon::{Imon, ImonConfig},
    load_acquisitor,
};

use self::time_series::{TimeSeriesGroupPaths, TimedEntry};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct TimeSeriesConfig {
    pub draw_valleys: bool,
    pub draw_valley_means: bool,
    pub draw_peaks: bool,
    pub draw_peak_means: bool,
    pub total_time: i64,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HandlerConfig {
    pub auto_save_path: PathBuf,
    pub wavelength_limits: Option<(f64, f64)>,
    pub power_limits: Option<(f64, f64)>,
    pub acquisitor: AcquisitorSimple,
    pub valley_detection: CriticalDetection,
    pub peak_detection: CriticalDetection,
    pub shadow_length: usize,
    pub time_series_config: TimeSeriesConfig,
}

#[derive(Debug)]
pub struct SpectrumHandler {
    config: Mutex<HandlerConfig>,
    pub last_spectrum: Arc<Mutex<Option<Spectrum>>>,
    pub frozen_spectra: Mutex<Vec<Spectrum>>,
    pub shadow_spectra: Mutex<Vec<Spectrum>>,
    pub time_series_group: Mutex<TimeSeriesGroup>,
    pub unread_spectrum: Arc<AtomicBool>,
    pub spectrum_limits: Mutex<Option<Limits>>, // 'Natural' limits
    pub log_sender: SyncSender<Log>,
    pub saving_new: Arc<AtomicBool>,
    pub acquisitor: Mutex<Acquisitor>,
}

#[derive(Debug, Serialize, Deserialize)]
pub enum State {
    Disconnected,
    Connected,
    Reading,
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum AcquisitorSimple {
    FileReader,
    Imon,
}

#[derive(Debug)]
pub enum Acquisitor {
    FileReader(FileReader),
    Imon(Imon),
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum AcquisitorConfig {
    FileReaderConfig(FileReaderConfig),
    ImonConfig(ImonConfig),
}

// Region: Acquisitor interface ------------------------------------------------

impl SpectrumHandler {
    pub fn connect(&self) -> Result<(), Box<dyn Error>> {
        let acquisitor = self.acquisitor.lock().unwrap();
        match &*acquisitor {
            Acquisitor::FileReader(file_reader) => file_reader.connect()?,
            Acquisitor::Imon(imon) => imon.connect()?,
        }

        Ok(())
    }

    pub fn disconnect(&self) -> Result<(), Box<dyn Error>> {
        let acquisitor = self.acquisitor.lock().unwrap();
        match &*acquisitor {
            Acquisitor::FileReader(file_reader) => file_reader.disconnect()?,
            Acquisitor::Imon(imon) => imon.disconnect()?,
        }

        Ok(())
    }

    pub fn start_reading(&self) -> Result<(), Box<dyn Error>> {
        let acquisitor = self.acquisitor.lock().unwrap();
        match &*acquisitor {
            Acquisitor::FileReader(file_reader) => file_reader.start_reading(self, false)?,
            Acquisitor::Imon(imon) => imon.start_reading(self, false)?,
        }

        Ok(())
    }

    pub fn read_single(&self) -> Result<(), Box<dyn Error>> {
        let acquisitor = self.acquisitor.lock().unwrap();
        match &*acquisitor {
            Acquisitor::FileReader(file_reader) => file_reader.start_reading(self, true)?,
            Acquisitor::Imon(imon) => imon.start_reading(self, true)?,
        }

        Ok(())
    }

    pub fn stop_reading(&self) -> Result<(), Box<dyn Error>> {
        let acquisitor = self.acquisitor.lock().unwrap();
        match &*acquisitor {
            Acquisitor::FileReader(file_reader) => file_reader.stop_reading()?,
            Acquisitor::Imon(imon) => imon.stop_reading()?,
        }

        Ok(())
    }

    pub fn get_state(&self) -> State {
        let acquisitor = self.acquisitor.lock().unwrap();
        match &*acquisitor {
            Acquisitor::FileReader(file_reader) => file_reader.get_simplified_state(),
            Acquisitor::Imon(imon) => imon.get_simplified_state(),
        }
    }
}

// Region: Basic stuff ---------------------------------------------------------

impl SpectrumHandler {
    pub fn get_last_spectrum_path(&self, svg_limits: (u32, u32)) -> Option<String> {
        if self.unread_spectrum.load(atomic::Ordering::Relaxed) {
            self.update_shadow_spectra();
            self.update_time_series();
        }

        let max_power = self.get_max_power();
        let spectrum = self.last_spectrum.lock().unwrap();

        if let Some(spectrum_limits) = self.get_limits(max_power) {
            self.unread_spectrum.store(false, atomic::Ordering::Relaxed);
            (*spectrum)
                .as_ref()
                .map(|spectrum| spectrum.to_path(svg_limits, &spectrum_limits))
        } else {
            None
        }
    }

    pub fn get_last_spectrum_valleys_points(
        &self,
        svg_limits: (u32, u32),
    ) -> Option<Vec<(f64, f64)>> {
        let max_power = self.get_max_power();
        let mut spectrum = self.last_spectrum.lock().unwrap();
        let config = self.config.lock().unwrap();
        let method = config.valley_detection.clone();
        drop(config); // Get_limits will also ask for it, making a *deadlock*

        if let Some(spectrum_limits) = self.get_limits(max_power) {
            (*spectrum)
                .as_mut()
                .map(|spectrum| spectrum.get_valleys_points(svg_limits, &spectrum_limits, method))
        } else {
            None
        }
    }

    pub fn get_last_spectrum_peaks_points(
        &self,
        svg_limits: (u32, u32),
    ) -> Option<Vec<(f64, f64)>> {
        let max_power = self.get_max_power();
        let mut spectrum = self.last_spectrum.lock().unwrap();
        let config = self.config.lock().unwrap();
        let method = config.peak_detection.clone();
        drop(config); // Get_limits will also ask for it, making a *deadlock*

        if let Some(spectrum_limits) = self.get_limits(max_power) {
            (*spectrum)
                .as_mut()
                .map(|spectrum| spectrum.get_peaks_points(svg_limits, &spectrum_limits, method))
        } else {
            None
        }
    }

    pub fn update_limits(&self) {
        let active_spectrum = self.last_spectrum.lock().unwrap();
        let frozen_spectra = self.frozen_spectra.lock().unwrap();
        let mut limits = self.spectrum_limits.lock().unwrap();

        if let Some(spectrum) = active_spectrum.as_ref() {
            let mut new_limits = frozen_spectra
                .iter()
                .map(|spectrum| &(spectrum.limits))
                .fold(spectrum.limits.clone(), |acc, new| Limits {
                    wavelength: (
                        acc.wavelength.0.min(new.wavelength.0),
                        acc.wavelength.1.max(new.wavelength.1),
                    ),
                    power: (acc.power.0.min(new.power.0), acc.power.1.max(new.power.1)),
                });

            new_limits.power.0 -= 3.0;
            new_limits.power.1 += 3.0;

            // To avoid jitter when small changes are made to the spectrum limits
            if let Some(old_limits) = (*limits).clone() {
                if new_limits.wavelength != old_limits.wavelength
                    || new_limits.power.0 < old_limits.power.0 - 2.0
                    || new_limits.power.1 > old_limits.power.1 + 2.0
                {
                    *limits = Some(new_limits);
                }
            } else {
                *limits = Some(new_limits);
            }
        }
    }

    pub fn get_limits(&self, power_offset: f64) -> Option<Limits> {
        let config = self.config.lock().unwrap();

        let default_limits = self.spectrum_limits.lock().unwrap();

        let default_limits = match default_limits.clone() {
            Some(limits) => limits,
            None => return None,
        };

        let limits_wl = match config.wavelength_limits {
            Some(limits) => limits,
            None => default_limits.wavelength,
        };

        let limits_pwr = match config.power_limits {
            Some(limits) => (limits.0 + power_offset, limits.1 + power_offset),
            None => default_limits.power,
        };

        Some(Limits {
            wavelength: limits_wl,
            power: limits_pwr,
        })
    }

    pub fn get_max_power(&self) -> f64 {
        let active_spectrum = self.last_spectrum.lock().unwrap();
        let frozen_spectra = self.frozen_spectra.lock().unwrap();

        frozen_spectra
            .iter()
            .chain(&*active_spectrum)
            .map(|spectrum| spectrum.limits.power.1)
            .fold(f64::NEG_INFINITY, |acc, new| acc.max(new))
    }

    pub fn get_valley_detection(&self) -> CriticalDetection {
        let config = self.config.lock().unwrap();
        config.valley_detection.clone()
    }

    pub fn get_peak_detection(&self) -> CriticalDetection {
        let config = self.config.lock().unwrap();
        config.peak_detection.clone()
    }

    pub fn get_time_series_config(&self) -> TimeSeriesConfig {
        let config = self.config.lock().unwrap();
        config.time_series_config.clone()
    }
}

// Region: Frozen stuff --------------------------------------------------------

impl SpectrumHandler {
    pub fn freeze_spectrum(&self) {
        let mut frozen_list = self.frozen_spectra.lock().unwrap();

        let mut spectrum = self.last_spectrum.lock().unwrap();

        match &*spectrum {
            Some(spectrum) => {
                frozen_list.push(spectrum.clone());
                self.log_info("[FFS] Congelando espectro".to_string());
            }
            None => self.log_war("[FFS] Sem espectro para congelar".to_string()),
        }

        *spectrum = None;
    }

    pub fn delete_frozen_spectrum(&self, id: usize) {
        let mut frozen_list = self.frozen_spectra.lock().unwrap();

        if id >= frozen_list.len() {
            self.log_error(
                "[FDF] Não foi possível deletar o congelado, id \
                fora dos limites"
                    .to_string(),
            );
            return;
        }

        frozen_list.remove(id);
        self.log_info(format!("[FDF] Deletando congelado {:02}", id));
    }

    pub fn get_frozen_spectrum_path(&self, id: usize, svg_limits: (u32, u32)) -> Option<String> {
        let max_power = self.get_max_power();
        let frozen_list = self.frozen_spectra.lock().unwrap();

        if id >= frozen_list.len() {
            self.log_error(
                "[FGF] Não foi possível pegar o espectro congelado, \
                id fora dos limites"
                    .to_string(),
            );
            return None;
        }

        let spectrum = &frozen_list[id];

        self.get_limits(max_power)
            .map(|spectrum_limits| spectrum.to_path(svg_limits, &spectrum_limits))
    }

    pub fn get_frozen_spectrum_valleys_points(
        &self,
        id: usize,
        svg_limits: (u32, u32),
    ) -> Option<Vec<(f64, f64)>> {
        let max_power = self.get_max_power();
        let mut frozen_list = self.frozen_spectra.lock().unwrap();
        let config = self.config.lock().unwrap();
        let method = config.valley_detection.clone();
        drop(config); // Get_limits will also ask for it, making a *deadlock*

        if id >= frozen_list.len() {
            self.log_error(
                "[FGF] Não foi possível pegar o espectro congelado, \
                id fora dos limites"
                    .to_string(),
            );
            return None;
        }

        let spectrum = &mut frozen_list[id];

        self.get_limits(max_power).map(|spectrum_limits| {
            spectrum.get_valleys_points(svg_limits, &spectrum_limits, method)
        })
    }

    pub fn get_frozen_spectrum_peaks_points(
        &self,
        id: usize,
        svg_limits: (u32, u32),
    ) -> Option<Vec<(f64, f64)>> {
        let max_power = self.get_max_power();
        let mut frozen_list = self.frozen_spectra.lock().unwrap();
        let config = self.config.lock().unwrap();
        let method = config.peak_detection.clone();
        drop(config); // Get_limits will also ask for it, making a *deadlock*

        if id >= frozen_list.len() {
            self.log_error(
                "[FGF] Não foi possível pegar o espectro congelado, \
                id fora dos limites"
                    .to_string(),
            );
            return None;
        }

        let spectrum = &mut frozen_list[id];

        self.get_limits(max_power)
            .map(|spectrum_limits| spectrum.get_peaks_points(svg_limits, &spectrum_limits, method))
    }

    pub fn get_frozen_length(&self) -> usize {
        let frozen_list = self.frozen_spectra.lock().unwrap();
        frozen_list.len()
    }

    pub fn clone_frozen(&self, id: usize) -> Option<Spectrum> {
        let frozen_list = self.frozen_spectra.lock().unwrap();

        if id >= frozen_list.len() {
            self.log_error(
                "[FCF] Não foi possível clonar o espectro \
            congelado, id fora dos limites"
                    .to_string(),
            );
            return None;
        }

        let spectrum = &frozen_list[id];
        Some(spectrum.clone())
    }

    pub fn save_frozen(&self, id: usize, path: &Path) {
        let frozen_list = self.frozen_spectra.lock().unwrap();

        if id >= frozen_list.len() {
            self.log_error(
                "[FSF] Não foi possível pegar o espectro congelado, \
                id fora dos limites"
                    .to_string(),
            );
            return;
        }

        let spectrum = &frozen_list[id];

        match spectrum.save(path) {
            Ok(_) => self.log_info(format!("[FSF] Espectro {} salvo", id)),
            Err(error) => self.log_error(format!(
                "[FSF] Falhou ao salvar \
                espectro {} ({})",
                id, error
            )),
        }
    }

    pub fn save_all_frozen_info(&self, path: &Path) -> Result<(), Box<dyn Error>> {
        let frozen_list = self.frozen_spectra.lock().unwrap();

        let infos: Vec<Info> = frozen_list
            .iter()
            .map(|spectrum| spectrum.info.clone())
            .enumerate()
            .map(|(id, mut info)| {
                if info.name.is_none() {
                    info.name = Some(format!("spectrum{:03}", id));
                }
                info
            })
            .collect();

        let infos_str = serde_json::to_string(&infos)?;
        let mut file = File::create(path)?;
        file.write_all(infos_str.as_bytes())?;

        Ok(())
    }
}

// Region: Shadow stuff --------------------------------------------------------

impl SpectrumHandler {
    fn update_shadow_spectra(&self) {
        let spectrum = self.last_spectrum.lock().unwrap();
        let mut shadow_list = self.shadow_spectra.lock().unwrap();
        let config = self.config.lock().unwrap();

        if let Some(spectrum) = spectrum.as_ref() {
            shadow_list.push(spectrum.clone());
        }

        // +1 because the last is in the list, but is not sent sa shadow
        while shadow_list.len() > config.shadow_length + 1 {
            shadow_list.remove(0);
        }
    }

    pub fn get_shadow_paths(&self, svg_limits: (u32, u32)) -> Vec<String> {
        let max_power = self.get_max_power();
        let spectrum_limits = self.get_limits(max_power);
        let shadow_list = self.shadow_spectra.lock().unwrap();

        if let Some(spectrum_limits) = spectrum_limits {
            (*shadow_list) // shadow_list goes from older to newer
                .iter()
                .rev()
                .skip(1) // Ignore the newest
                .rev()
                .map(|spectrum| spectrum.to_path(svg_limits, &spectrum_limits))
                .collect()
        } else {
            vec![]
        }
    }
}

// Region: Time Series stuff

impl SpectrumHandler {
    fn update_time_series(&self) {
        let mut spectrum = self.last_spectrum.lock().unwrap();
        let config = self.config.lock().unwrap();
        let valley_method = config.valley_detection.clone();
        let peak_method = config.peak_detection.clone();

        let spectrum = if let Some(spectrum) = (*spectrum).as_mut() {
            spectrum
        } else {
            return;
        };

        let valleys = spectrum.get_valleys(valley_method);
        let valleys_mean = if !valleys.is_empty() {
            vec![
                valleys
                    .iter()
                    .fold(0.0, |acc, valley| acc + valley.wavelength)
                    / valleys.len() as f64,
            ]
        } else {
            vec![]
        };

        let peaks = spectrum.get_peaks(peak_method);
        let peaks_mean = if !peaks.is_empty() {
            vec![peaks.iter().fold(0.0, |acc, peak| acc + peak.wavelength) / peaks.len() as f64]
        } else {
            vec![]
        };

        let timestamp = Local::now().timestamp_millis();
        let cvt_entry = |spec_value: &SpectrumValue| TimedEntry {
            value: spec_value.wavelength,
            timestamp,
        };

        let valleys = valleys.iter().map(cvt_entry).collect::<Vec<TimedEntry>>();
        let valleys_mean = valleys_mean
            .iter()
            .map(|&value| TimedEntry { value, timestamp })
            .collect::<Vec<TimedEntry>>();

        let peaks = peaks.iter().map(cvt_entry).collect::<Vec<TimedEntry>>();
        let peaks_mean = peaks_mean
            .iter()
            .map(|&value| TimedEntry { value, timestamp })
            .collect::<Vec<TimedEntry>>();

        let mut time_series = self.time_series_group.lock().unwrap();

        time_series.valleys.push_batch(&valleys);
        time_series.valley_means.push_batch(&valleys_mean);
        time_series.peaks.push_batch(&peaks);
        time_series.peak_means.push_batch(&peaks_mean);
    }

    pub fn get_time_series_paths(&self, svg_limits: (u32, u32)) -> TimeSeriesGroupPaths {
        let graph_limits = self.get_limits(0.0);
        if let Some(graph_limits) = graph_limits {
            let time_series = self.time_series_group.lock().unwrap();
            let config = self.config.lock().unwrap();

            time_series.to_path(svg_limits, &graph_limits, &config.time_series_config)
        } else {
            TimeSeriesGroupPaths::empty()
        }
    }
}

// Region: Loggers -------------------------------------------------------------

impl SpectrumHandler {
    pub fn log_info(&self, msg: String) {
        log_info(&self.log_sender, msg);
    }

    pub fn log_war(&self, msg: String) {
        log_war(&self.log_sender, msg);
    }

    pub fn log_error(&self, msg: String) {
        log_error(&self.log_sender, msg);
    }
}

// Region: Config --------------------------------------------------------------

pub fn default_config() -> HandlerConfig {
    HandlerConfig {
        auto_save_path: "D:/test/save".to_string().into(),
        wavelength_limits: None,
        power_limits: None,
        acquisitor: AcquisitorSimple::FileReader,
        valley_detection: CriticalDetection::Lorentz { prominence: 3.0 },
        peak_detection: CriticalDetection::None,
        shadow_length: 10,
        time_series_config: TimeSeriesConfig {
            draw_valleys: true,
            draw_valley_means: false,
            draw_peaks: false,
            draw_peak_means: false,
            total_time: 300,
        },
    }
}

impl SpectrumHandler {
    pub fn update_config(&self, new_config: HandlerConfig) {
        let mut config = self.config.lock().unwrap();

        if new_config.acquisitor != config.acquisitor {
            let mut acquisitor = self.acquisitor.lock().unwrap();
            let new_acquisitor = load_acquisitor(&new_config.acquisitor, self.log_sender.clone());

            *acquisitor = new_acquisitor;
        }

        *config = new_config;
    }

    pub fn get_config(&self) -> HandlerConfig {
        let config = self.config.lock().unwrap();

        (*config).clone()
    }

    pub fn update_acquisitor_config(&self, new_config: AcquisitorConfig) {
        let acquisitor = self.acquisitor.lock().unwrap();

        match &*acquisitor {
            Acquisitor::FileReader(file_reader) => {
                if let AcquisitorConfig::FileReaderConfig(new_config) = new_config {
                    file_reader.update_config(new_config);
                } else {
                    self.log_error(
                        "[HUQ] Configuração incompatível, era esperado \
                        receber FileReaderConfig"
                            .to_string(),
                    );
                }
            }
            Acquisitor::Imon(imon) => {
                if let AcquisitorConfig::ImonConfig(new_config) = new_config {
                    imon.update_config(new_config);
                } else {
                    self.log_error(
                        "[HUQ] Configuração incompatível, era esperado \
                        receber ImonConfig"
                            .to_string(),
                    );
                }
            }
        }
    }

    pub fn get_acquisitor_config(&self) -> AcquisitorConfig {
        let acquisitor = self.acquisitor.lock().unwrap();

        match &*acquisitor {
            Acquisitor::FileReader(file_reader) => {
                AcquisitorConfig::FileReaderConfig(file_reader.get_config())
            }
            Acquisitor::Imon(imon) => AcquisitorConfig::ImonConfig(imon.get_config()),
        }
    }
}

// Region: Outside the impls ---------------------------------------------------

pub fn new_spectrum_handler(config: HandlerConfig, log_sender: SyncSender<Log>) -> SpectrumHandler {
    let acquisitor = load_acquisitor(&config.acquisitor, log_sender.clone());

    SpectrumHandler {
        config: Mutex::new(config),
        last_spectrum: Arc::new(Mutex::new(None)),
        frozen_spectra: Mutex::new(vec![]),
        shadow_spectra: Mutex::new(vec![]),
        time_series_group: Mutex::new(TimeSeriesGroup::empty()),
        unread_spectrum: Arc::new(AtomicBool::new(false)),
        spectrum_limits: Mutex::new(None),
        log_sender,
        saving_new: Arc::new(AtomicBool::new(false)),
        acquisitor: Mutex::new(acquisitor),
    }
}
