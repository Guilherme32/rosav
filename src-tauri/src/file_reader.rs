#![allow(dead_code)]

use crate::{ Log, log_info, log_error, log_war };

use std::path::Path;
use notify;
use notify::{ Watcher, RecursiveMode };
use std::fs::{ self, File };
use std::io::Read;
use std::io;
use std::error::Error;

use std::time::Duration;
use std::thread::sleep;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{ self, AtomicBool };
use std::sync::mpsc::SyncSender;

use serde::{Serialize, Deserialize};

use crate::spectrum::*;

pub fn test() {
    println!("funcao de teste");
    return ();
}

// #[derive(Debug)]
// pub struct Connected;
// #[derive(Debug)]
// pub struct Disconnected;
// #[derive(Debug)]
// pub struct Continuous {
//     watcher: notify::RecommendedWatcher
// }

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileReaderConfig {
    pub auto_save_path: String,
    pub watcher_path: String,
    pub wavelength_limits: Option<(f64, f64)>,
    pub power_limits: Option<(f64, f64)>,
}

#[derive(Debug)]
pub enum ReaderState {
    Disconnected,
    Connected,
    Reading (notify::RecommendedWatcher)
}

#[derive(Debug)]
pub struct FileReader {
    pub config: Mutex<FileReaderConfig>,
    pub last_spectrum: Arc<Mutex<Option<Spectrum>>>,
    pub frozen_spectra: Mutex<Vec<Spectrum>>,
    pub unread_spectrum: Arc<AtomicBool>,
    pub spectrum_limits: Mutex<Option<Limits>>,
    pub log_sender: SyncSender<Log>,
    pub saving_new: Arc<AtomicBool>,
    pub state: Arc<Mutex<ReaderState>>
}

#[derive(Debug)]
pub enum ConnectError {
    LockFailed,
    ReaderAlreadyConnected,
    PathDoesNotExist,
    PathIsNotDir,
    PathWithoutPermission
}

#[derive(Debug)]
pub enum ContinuousError {
    LockFailed,
    ReaderNotConnected,
    ReaderAlreadyContinuous,
    NotifyInternalError
}

impl FileReader {
    pub fn connect<'a>(&'a self) -> Result<(), ConnectError>
    {
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(_) => {
                self.log_error("[FCN] Falha ao obter lock para 'state'".to_string());
                return Err(ConnectError::LockFailed)
            }
        };

        match *state {
            ReaderState::Disconnected => (),
            _ => {
                self.log_war("[FCN] Não foi possível conectar: O aquisitor já \
                    está conectado".to_string());
                return Err(ConnectError::ReaderAlreadyConnected);
            }
        }

        let config = match self.config.lock() {
            Ok(config) => config,
            Err(_) => {
                self.log_error("[FCN] Falha ao obter lock para 'config'".to_string());
                return Err(ConnectError::LockFailed)
            }
        };
        let path = Path::new(&config.watcher_path);

        match path.try_exists() {
            Err(_) => { 
                self.log_war("[FCN] Não foi possível conectar: A permissão para
                    acessar o caminho configurado foi negada".to_string());
                return Err(ConnectError::PathWithoutPermission);
            },
            Ok(exists) => {
                if !exists {
                    self.log_war("[FCN] Não foi possível conectar: O caminho \
                        configurado não existe".to_string());
                    return Err(ConnectError::PathDoesNotExist);
                }
            }
        }

        if !path.is_dir() {
            self.log_war("[FCN] Não foi possível conectar: O caminho \
                configurado não é uma pasta".to_string());
            return Err(ConnectError::PathIsNotDir);
        }

        *state = ReaderState::Connected;
        self.log_info("[FCN] Aquisitor conectado".to_string());
        Ok(())
    }

    pub fn disconnect<'a>(&'a self) -> Result<(), &'static str>
    {
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(_) => {
                self.log_error("[FDN] Falha ao obter lock para 'state'".to_string());
                return Err("Lock acquisition failed");
            }
        };

        match *state {
            ReaderState::Disconnected => {
                self.log_war("[FDN] Não foi possível desconectar: Aquisitor \
                    já está desconectado".to_string());
                return Err("Already disconnected");
            },
            _ => ()
        }

        *state = ReaderState::Disconnected;
        self.log_info("[FDN] Aquisitor desconectado".to_string());
        Ok(())
    }

    pub fn start_reading<'a>(&'a self) -> Result<(), ContinuousError>
    {
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(_) => {
                self.log_error("[FSR] Falha ao obter lock para 'state'".to_string());
                return Err(ContinuousError::LockFailed)
            }
        };

        match *state {
            ReaderState::Disconnected => return Err(ContinuousError::ReaderNotConnected),
            ReaderState::Reading(_) => return Err(ContinuousError::ReaderAlreadyContinuous),
            _ => ()
        }

        let config = match self.config.lock() {
            Ok(config) => config,
            Err(_) => {
                self.log_error("[FSR] Falha ao obter lock para 'config'".to_string());
                return Err(ContinuousError::LockFailed)
            }
        };
        let watcher_path = Path::new(&config.watcher_path);
        let auto_save_path = config.auto_save_path.clone();

        let spectrum_reference = Arc::clone(&self.last_spectrum);
        let flag_reference = Arc::clone(&self.unread_spectrum);
        let saving_reference = Arc::clone(&self.saving_new);
        let log_sender_clone = Arc::new(self.log_sender.clone());
        let state_reference = Arc::clone(&self.state);

        let callback = move |event| match watcher_callback(
            event,
            Arc::clone(&spectrum_reference),
            Arc::clone(&flag_reference),
            Arc::clone(&saving_reference),
            &auto_save_path,
            Arc::clone(&log_sender_clone)
        ) {
            Ok(_) => (),
            Err(_) => {
                if let Ok(mut state) = state_reference.lock() {
                    *state = ReaderState::Disconnected;
                    log_war(&log_sender_clone, "[FSR] Aquisitor desconectado \
                        devido a um erro".to_string());
                }
                ()
            }
        };

        let mut watcher = match notify::recommended_watcher(callback) {
            Ok(_watcher) => _watcher,
            Err(_) => return Err(ContinuousError::NotifyInternalError)
        };

        match watcher.watch(watcher_path, RecursiveMode::NonRecursive) {
            Ok(_) => (),
            Err(_) => return Err(ContinuousError::NotifyInternalError)
        }

        *state = ReaderState::Reading(watcher);
        self.log_info("[FSR] Aquisitor lendo".to_string());
        Ok(())
    }

    pub fn stop_reading<'a>(&'a self) -> Result<(), &'static str>
    {
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(_) => {
                self.log_error("[FTP] Falha ao obter lock para 'state'".to_string());
                return Err("Lock acquisition failed");
            }
        };

        match *state {
            ReaderState::Reading(_) => (),
            _ => {
                self.log_war("[FTP] Não foi possível parar de ler, o aquisitor \
                    não estava lendo".to_string());
                return Err("Invalid State: Not reading");
            }
        }

        *state = ReaderState::Connected;
        self.log_info("[FTP] Aquisitor parou de ler".to_string());
        Ok(())
    }


    pub fn get_last_spectrum_path(&self, svg_limits: (u32, u32)) -> Option<String> {
        let spectrum = match self.last_spectrum.lock() {
            Ok(spectrum) => spectrum,
            Err(_) => { 
                self.log_error("[FGL] Falha ao obter lock para 'last_spectrum'".to_string());
                return None;
            }
        };

        let spec_limits = self.get_limits();

        if let Some(spec_limits) = spec_limits {
            self.unread_spectrum.store(false, atomic::Ordering::Relaxed);
            match &*spectrum {
                Some(spectrum) => Some(spectrum.to_path(svg_limits, &spec_limits)),
                None => None
            }
        } else {
            None
        }
    }

    pub fn update_limits(&self) {
        let spectrum = match self.last_spectrum.lock() {
            Ok(spectrum) => spectrum,
            Err(_) => { 
                self.log_error("[FUL] Falha ao obter lock para 'last_spectrum'".to_string());
                return ();
            }
        };

        let mut limits = match self.spectrum_limits.lock() {
            Ok(limits) => limits,
            Err(_) => { 
                self.log_error("[FUL] Falha ao obter lock para 'spectrum_limits'".to_string());
                return ();
            }
        };

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
        let config = match self.config.lock() {
            Ok(config) => config,
            Err(_) => {
                self.log_error("[FGL] Falha ao obter lock para 'config'".to_string());
                return None
            }
        };
        
        let default_limits = match self.spectrum_limits.lock() {
            Ok(spec_limits) => spec_limits,
            Err(_) => { 
                self.log_error("[FGL] Falha ao obter lock para 'spectrum_limits'".to_string());
                return None;
            }
        };

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

    pub fn freeze_spectrum(&self) {
        let mut frozen_list = match self.frozen_spectra.lock() {
            Ok(spectra) => spectra,
            Err(_) => { 
                self.log_error("[FFS] Falha ao obter lock para os congelados".to_string());
                return ();
            }
        };

        let spectrum = match self.last_spectrum.lock() {
            Ok(spectrum) => spectrum,
            Err(_) => { 
                self.log_error("[FFS] Falha ao obter lock para 'last_spectrum'".to_string());
                return ();
            }
        };

        match &*spectrum {
            Some(spectrum) => { 
                frozen_list.push(spectrum.clone());
                self.log_info("[FFS] Congelando espectro".to_string());
            },
            None => self.log_war("[FFS] Sem espectro para congelar".to_string())
        }
    }

    pub fn delete_frozen_spectrum(&self, id: usize) {
        let mut frozen_list = match self.frozen_spectra.lock() {
            Ok(spectra) => spectra,
            Err(_) => { 
                self.log_error("[FDF] Falha ao obter lock para os congelados".to_string());
                return ();
            }
        };

        if id >= frozen_list.len() {
            self.log_error("[FDF] Não foi possível deletar o congelado, id fora \
                dos limites".to_string());
            return ();
        }

        frozen_list.remove(id);
        self.log_info(format!("[FDF] Deletando congelado {:02}", id));
    }

    pub fn get_frozen_spectrum_path(&self, id: usize, svg_limits: (u32, u32)) -> Option<String> {
        let frozen_list = match self.frozen_spectra.lock() {
            Ok(spectra) => spectra,
            Err(_) => { 
                self.log_error("[FGF] Falha ao obter lock para os congelados".to_string());
                return None;
            }
        };

        if id >= frozen_list.len() {
            self.log_error("[FGF] Não foi possível pegar o espectro congelado, \
                id fora dos limites".to_string());
            return None;
        }

        let spectrum = &frozen_list[id];

        let spec_limits = match self.spectrum_limits.lock() {
            Ok(spec_limits) => spec_limits,
            Err(_) => { 
                self.log_error("[FGF] Falha ao obter lock para 'spectrum_limits'".to_string());
                return None;
            }
        };

        if let Some(spec_limits) = &*spec_limits {
            Some(spectrum.to_path(svg_limits, spec_limits))
        } else {
            None
        }
    }

    pub fn clone_frozen(&self, id: usize) -> Option<Spectrum> {
        let frozen_list = match self.frozen_spectra.lock() {
            Ok(spectra) => spectra,
            Err(_) => { 
                self.log_error("[FCF] Falha ao obter lock para os congelados".to_string());
                return None;
            }
        };

        if id >= frozen_list.len() {
            self.log_error("[FCF] Não foi possível clonar o espectro congelado, \
            id fora dos limites".to_string());
            return None;
        }

        let spectrum = &frozen_list[id];
        Some(spectrum.clone())
    }

    pub fn save_frozen(&self, id: usize, path: &Path) {
        let frozen_list = match self.frozen_spectra.lock() {
            Ok(spectra) => spectra,
            Err(_) => { 
                self.log_error("[FSF] Falha ao obter lock para os congelados".to_string());
                return ();
            }
        };

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

pub fn new_file_reader(config: FileReaderConfig, log_sender: SyncSender<Log>) -> FileReader {
    FileReader {
        config: Mutex::new(config),
        last_spectrum: Arc::new(Mutex::new(None)),
        frozen_spectra: Mutex::new(vec![]),
        unread_spectrum: Arc::new(AtomicBool::new(false)),
        spectrum_limits: Mutex::new(None),
        log_sender,
        saving_new: Arc::new(AtomicBool::new(false)),
        state: Arc::new(Mutex::new(ReaderState::Disconnected))
    }
}

fn read_file_event(event: &notify::Event) -> Result<String, Box<dyn Error>> {
    let path = &event.paths[0];

    for _ in 0..10 {
        let mut file = match File::open(path) {                // Tries to open the file
            Ok(_file) => _file,                                // Will retry 10 times if
            Err(err) if err.raw_os_error() == Some(32) => {    // it can't open because someone
                sleep(Duration::from_millis(100));            // else is using it (os err 32)
                continue;
            },
            Err(err) => { return Err(Box::new(err)); }
        };

        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        return Ok(contents);
    }

    // The only way it gets here is if error 32 happened 10 times in a row
    return Err(Box::new(io::Error::from_raw_os_error(32)));
}

fn watcher_callback<T: std::fmt::Debug>(
    response: Result<notify::Event, T>,
    last_spectrum: Arc<Mutex<Option<Spectrum>>>,
    new_spectrum: Arc<AtomicBool>,
    saving: Arc<AtomicBool>,
    auto_save_path: &str,
    log_tx: Arc<SyncSender<Log>>
) -> Result<(), ()> {
    let event = match response {
        Ok(event) if event.kind.is_create() => event,
        Ok(_) => return Ok(()),                                // Don't care about successfull non create events
        Err(error) => {
            log_error(&log_tx, format!("[FWC] Erro do 'watcher': {:?}", error));
            return Err(());
        }
    };

    let text = match read_file_event(&event) {
        Ok(text) => text,
        Err(error) => {
            log_error(&log_tx, format!("[FWC] Não foi possível responder ao \
                'file event': {:?}, \nErro: {}", event, error));
            return Err(());
        }
    };

    let spectrum = match Spectrum::from_str(&text) {
        Ok(spectrum) => spectrum,
        Err(error) => {
            log_error(&log_tx, format!("[FWC] Não foi posível transformar o \
                arquivo em um espectro ({})", error));
            return Err(());
        }
    };

    if saving.load(atomic::Ordering::Relaxed) {
        match auto_save_spectrum(&spectrum, auto_save_path) {
            Ok(num) => log_info(&log_tx, format!("[FWC] Espectro {:03} salvo", num)),
            Err(error) => log_error(&log_tx, format!("[FWC] Não foi possível \
                salvar o espectro novo ({})", error))
        }
    }

    match last_spectrum.lock() {
        Ok(mut last_spectrum) => {
            *last_spectrum = Some(spectrum);
            new_spectrum.store(true, atomic::Ordering::Relaxed);
        },
        Err(_) => {
            log_error(&log_tx, "[FWC] Falha ao adquirir lock para 'spectrum'".to_string());
            return Err(());
        }
    };

    Ok(())
}

fn auto_save_spectrum(spectrum: &Spectrum, folder_path: &str) -> Result<u32, Box<dyn Error>> {
    let folder_path = Path::new(folder_path);
    fs::create_dir_all(folder_path)?;

    for i in 0..100_000 {
        let new_path = folder_path.join(format!("spectrum{:03}.txt", i));
        if !new_path.exists() {
            spectrum.save(&new_path)?;
            return Ok(i);
        }
    } 

    Err(Box::new(io::Error::new(io::ErrorKind::Other, "Overflow de espectros,\
        o programa só suporta até 'spectrum99999'")))
}


// region Config -----------------------------------------------------------------------------------

impl FileReader {
    pub fn update_config(&self, new_config: FileReaderConfig) {
        let mut config = match self.config.lock() {
            Ok(config) => config,
            Err(_) => {
                self.log_error("[FUC] Falha ao adquirir lock para 'config".to_string());
                return ()
            }
        };

        *config = new_config;
    }
}
