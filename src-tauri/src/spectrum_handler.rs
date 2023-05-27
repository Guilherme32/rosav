
use crate::{ Log, log_info, log_error, log_war };

use std::path::Path;
use std::error::Error;

use std::sync::{Arc, Mutex};
use std::sync::atomic::{ self, AtomicBool };
use std::sync::mpsc::SyncSender;

use std::path::PathBuf;

use serde::{Serialize, Deserialize};

use crate::spectrum::*;

pub mod acquisitors;

use acquisitors::{
    file_reader::{ FileReader, FileReaderConfig },
    load_acquisitor
};


#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct HandlerConfig {
    pub auto_save_path: PathBuf,
    pub wavelength_limits: Option<(f64, f64)>,
    pub power_limits: Option<(f64, f64)>,
    pub acquisitor: AcquisitorSimple
}

#[derive(Debug)]
pub struct SpectrumHandler {
    config: Mutex<HandlerConfig>,
    pub last_spectrum: Arc<Mutex<Option<Spectrum>>>,
    pub frozen_spectra: Mutex<Vec<Spectrum>>,
    pub unread_spectrum: Arc<AtomicBool>,
    pub spectrum_limits: Mutex<Option<Limits>>,            // 'Natural' limits
    pub log_sender: SyncSender<Log>,
    pub saving_new: Arc<AtomicBool>,
    pub acquisitor: Mutex<Acquisitor>
}

#[derive(Debug, Serialize, Deserialize)]
pub enum State {
    Disconnected,
    Connected,
    Reading
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum AcquisitorSimple {
    FileReader
}

#[derive(Debug)]
pub enum Acquisitor {
    FileReader(FileReader)
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum AcquisitorConfig {
    FileReaderConfig(FileReaderConfig),
    Other(u32)                            // TODO just here to supress a warning, remove when adding others
}


// Region: Acquisitor interface ------------------------------------------------

impl SpectrumHandler {
    pub fn connect(&self) -> Result<(), Box<dyn Error>> {
        let acquisitor = self.acquisitor.lock().unwrap();
        match &*acquisitor {
            Acquisitor::FileReader(file_reader) => file_reader.connect()?
        }

        Ok(())
    }

    pub fn disconnect(&self) -> Result<(), Box<dyn Error>> {
        let acquisitor = self.acquisitor.lock().unwrap();
        match &*acquisitor {
            Acquisitor::FileReader(file_reader) => file_reader.disconnect()?
        }

        Ok(())
    }

    pub fn start_reading(&self) -> Result<(), Box<dyn Error>> {
        let acquisitor = self.acquisitor.lock().unwrap();
        match &*acquisitor {
            Acquisitor::FileReader(file_reader) => file_reader.start_reading(self)?
        }

        Ok(())
    }

    pub fn stop_reading(&self) -> Result<(), Box<dyn Error>> {
        let acquisitor = self.acquisitor.lock().unwrap();
        match &*acquisitor {
            Acquisitor::FileReader(file_reader) => file_reader.stop_reading()?
        }

        Ok(())
    }

    pub fn get_state(&self) -> State {
        let acquisitor = self.acquisitor.lock().unwrap();
        match &*acquisitor {
            Acquisitor::FileReader(file_reader) => file_reader.get_simplified_state()
        }
    }
}


// Region: Basic stuff ---------------------------------------------------------

impl SpectrumHandler {
    pub fn get_last_spectrum_path(
        &self,
        svg_limits: (u32, u32)
    ) -> Option<String> {
        let spectrum = self.last_spectrum.lock().unwrap();

        if let Some(spectrum_limits) = self.get_limits() {
            self.unread_spectrum.store(false, atomic::Ordering::Relaxed);
            match &*spectrum {
                Some(spectrum) =>
                    Some(spectrum.to_path(svg_limits, &spectrum_limits)),
                None =>
                    None
            }
        } else {
            None
        }
    }

    pub fn update_limits(&self) {
        let spectrum = self.last_spectrum.lock().unwrap();
        let mut limits = self.spectrum_limits.lock().unwrap();

        if let Some(spectrum) = &*spectrum {
            let new_limits = spectrum.get_limits();

            if let Some(_limits) = &*limits {
                let (mut new_wl_min, mut new_wl_max) = new_limits.wavelength;
                let (mut new_pwr_min, mut new_pwr_max) = new_limits.power;

                if _limits.wavelength.0 < new_wl_min {
                    new_wl_min = _limits.wavelength.0;
                }
                if _limits.wavelength.1 > new_wl_max {
                    new_wl_max = _limits.wavelength.1;
                }
                if _limits.power.0 < new_pwr_min {
                    new_pwr_min = _limits.power.0;
                }
                if _limits.power.1 > new_pwr_max {
                    new_pwr_max = _limits.power.1;
                }

                *limits = Some(Limits {
                    wavelength: (new_wl_min, new_wl_max),
                    power: (new_pwr_min, new_pwr_max)
                });
            } else {
                *limits = Some(new_limits);
            }
        }
    }

    pub fn get_limits(&self) -> Option<Limits> {
        let config = self.config.lock().unwrap();
        
        let default_limits = self.spectrum_limits.lock().unwrap();

        let default_limits = match default_limits.clone() {
            Some(limits) => limits,
            None => return None
        };

        let limits_wl = match config.wavelength_limits {
            Some(limits) => limits,
            None => default_limits.wavelength
        };

        let limits_pwr = match config.power_limits {
            Some(limits) => limits,
            None => default_limits.power
        };

        Some(Limits {
            wavelength: limits_wl,
            power: limits_pwr
        })
    }
    
}


// Region: Frozen stuff ---------------------------------------------------------

impl SpectrumHandler {
    pub fn freeze_spectrum(&self) {
        let mut frozen_list = self.frozen_spectra.lock().unwrap();

        let spectrum = self.last_spectrum.lock().unwrap();

        match &*spectrum {
            Some(spectrum) => { 
                frozen_list.push(spectrum.clone());
                self.log_info("[FFS] Congelando espectro".to_string());
            },
            None => self.log_war("[FFS] Sem espectro para congelar".to_string())
        }
    }

    pub fn delete_frozen_spectrum(&self, id: usize) {
        let mut frozen_list = self.frozen_spectra.lock().unwrap();

        if id >= frozen_list.len() {
            self.log_error("[FDF] Não foi possível deletar o congelado, id \
                fora dos limites".to_string());
            return ();
        }

        frozen_list.remove(id);
        self.log_info(format!("[FDF] Deletando congelado {:02}", id));
    }

    pub fn get_frozen_spectrum_path(
        &self,
        id: usize,
        svg_limits: (u32, u32)
    ) -> Option<String> {
        let frozen_list = self.frozen_spectra.lock().unwrap();

        if id >= frozen_list.len() {
            self.log_error("[FGF] Não foi possível pegar o espectro congelado, \
                id fora dos limites".to_string());
            return None;
        }

        let spectrum = &frozen_list[id];

        if let Some(spectrum_limits) = self.get_limits() {
            Some(spectrum.to_path(svg_limits, &spectrum_limits))
        } else {
            None
        }
    }

    pub fn clone_frozen(&self, id: usize) -> Option<Spectrum> {
        let frozen_list = self.frozen_spectra.lock().unwrap();

        if id >= frozen_list.len() {
            self.log_error("[FCF] Não foi possível clonar o espectro \
            congelado, id fora dos limites".to_string());
            return None;
        }

        let spectrum = &frozen_list[id];
        Some(spectrum.clone())
    }
    
    pub fn save_frozen(&self, id: usize, path: &Path) {
        let frozen_list = self.frozen_spectra.lock().unwrap();

        if id >= frozen_list.len() {
            self.log_error("[FSF] Não foi possível pegar o espectro congelado, \
                id fora dos limites".to_string());
            return ();
        }

        let spectrum = &frozen_list[id];

        match spectrum.save(path) {
            Ok(_) => self.log_info(format!("[FSF] Espectro {} salvo", id)),
            Err(error) => self.log_error(format!("[FSF] Falhou ao salvar \
                espectro {} ({})", id, error))
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
        acquisitor: AcquisitorSimple::FileReader
    }
}

impl SpectrumHandler {
    pub fn update_config(&self, new_config: HandlerConfig) {
        let mut config = self.config.lock().unwrap();

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
                    self.log_error("[HUQ] Configuração incompatível, era esperado \
                        receber FileReaderConfig".to_string());
                }
            }
        }
    }

    pub fn get_acquisitor_config(&self) -> AcquisitorConfig {
        let acquisitor = self.acquisitor.lock().unwrap();

        match &*acquisitor {
            Acquisitor::FileReader(file_reader) => 
                AcquisitorConfig::FileReaderConfig(file_reader.get_config())
        }
    }
}


// Region: Outside the impls ---------------------------------------------------

pub fn new_spectrum_handler(
    config: HandlerConfig,
    log_sender: SyncSender<Log>
) -> SpectrumHandler {
    let acquisitor = load_acquisitor(&config.acquisitor, log_sender.clone());

    SpectrumHandler {
        config: Mutex::new(config),
        last_spectrum: Arc::new(Mutex::new(None)),
        frozen_spectra: Mutex::new(vec![]),
        unread_spectrum: Arc::new(AtomicBool::new(false)),
        spectrum_limits: Mutex::new(None),
        log_sender,
        saving_new: Arc::new(AtomicBool::new(false)),
        acquisitor: Mutex::new(acquisitor)
    }
}

