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

#[derive(Debug)]
pub enum ReaderState {
    Disconnected,
    Connected,
    Reading (notify::RecommendedWatcher)
}

#[derive(Debug)]
pub struct FileReader {
    pub path: String,
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
                self.log_error("[FCN] Failed to acquire state lock".to_string());
                return Err(ConnectError::LockFailed)
            }
        };

        match *state {
            ReaderState::Disconnected => (),
            _ => {
                self.log_war("[FCN] Could not connect: the acquisitor is \
                    already connected".to_string());
                return Err(ConnectError::ReaderAlreadyConnected);
            }
        }

        let path = Path::new(&self.path);

        match path.try_exists() {
            Err(_) => { 
                self.log_war("[FCN] Could not connect: the permission to \
                    the path was denied".to_string());
                return Err(ConnectError::PathWithoutPermission);
            },
            Ok(exists) => {
                if !exists {
                    self.log_war("[FCN] Could not connect: the configured \
                        path does not exist".to_string());
                    return Err(ConnectError::PathDoesNotExist);
                }
            }
        }

        if !path.is_dir() {
            self.log_war("[FCN] Could not connect: the configured \
                path is not a directory".to_string());
            return Err(ConnectError::PathIsNotDir);
        }

        *state = ReaderState::Connected;
        self.log_info("[FCN] Acquisitor connected".to_string());
        Ok(())
    }

    pub fn disconnect<'a>(&'a self) -> Result<(), &'static str>
    {
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(_) => {
                self.log_error("[FDN] Failed to acquire state lock".to_string());
                return Err("Lock acquisition failed");
            }
        };

        match *state {
            ReaderState::Disconnected => {
                self.log_war("[FDN] Could not disconnect: the acquisitor is \
                    already disconnected".to_string());
                return Err("Already disconnected");
            },
            _ => ()
        }

        *state = ReaderState::Disconnected;
        self.log_info("[FDN] Acquisitor disconnected".to_string());
        Ok(())
    }

    pub fn start_reading<'a>(&'a self) -> Result<(), ContinuousError>
    {
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(_) => {
                self.log_error("[FSR] Failed to acquire state lock".to_string());
                return Err(ContinuousError::LockFailed)
            }
        };

        match *state {
            ReaderState::Disconnected => return Err(ContinuousError::ReaderNotConnected),
            ReaderState::Reading(_) => return Err(ContinuousError::ReaderAlreadyContinuous),
            _ => ()
        }

        let path = Path::new(&self.path);

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
            Arc::clone(&log_sender_clone)
        ) {
            Ok(_) => (),
            Err(_) => {                // TODO logar a desconexão
                if let Ok(mut state) = state_reference.lock() {
                    *state = ReaderState::Disconnected;
                    log_war(&log_sender_clone, "[FSR] Acquisition backend \
                        disconnected due to an error".to_string());
                }
                ()
            }
        };

        let mut watcher = match notify::recommended_watcher(callback) {
            Ok(_watcher) => _watcher,
            Err(_) => return Err(ContinuousError::NotifyInternalError)
        };

        match watcher.watch(path, RecursiveMode::NonRecursive) {
            Ok(_) => (),
            Err(_) => return Err(ContinuousError::NotifyInternalError)
        }

        *state = ReaderState::Reading(watcher);
        self.log_info("[FSR] Acquisitor reading".to_string());
        Ok(())
    }

    pub fn stop_reading<'a>(&'a self) -> Result<(), &'static str>
    {
        let mut state = match self.state.lock() {
            Ok(state) => state,
            Err(_) => {
                self.log_error("[FTP] Failed to acquire state lock".to_string());
                return Err("Lock acquisition failed");
            }
        };

        match *state {
            ReaderState::Reading(_) => (),
            _ => {
                self.log_war("[FTP] Could not stop reading: the acquisitor \
                    was not reading".to_string());
                return Err("Invalid State: Not reading");
            }
        }

        *state = ReaderState::Connected;
        self.log_info("[FTP] Acq. stopped reading".to_string());
        Ok(())
    }


    pub fn get_last_spectrum_path(&self, svg_limits: (u32, u32)) -> Option<String> {
        let spectrum = match self.last_spectrum.lock() {
            Ok(spectrum) => spectrum,
            Err(_) => { 
                self.log_error("[FGL] Could not acquire the lock to get last spectrum".to_string());
                return None;
            }
        };

        let spec_limits = match self.spectrum_limits.lock() {
            Ok(spec_limits) => spec_limits,
            Err(_) => { 
                self.log_error("[FUL] Could not acquire the lock to get last limits".to_string());
                return None;
            }
        };

        if let Some(spec_limits) = &*spec_limits {
            self.unread_spectrum.store(false, atomic::Ordering::Relaxed);
            match &*spectrum {
                Some(spectrum) => Some(spectrum.to_path(svg_limits, spec_limits)),
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
                self.log_error("[FUL] Could not acquire the lock to get last spectrum".to_string());
                return ();
            }
        };

        let mut limits = match self.spectrum_limits.lock() {
            Ok(limits) => limits,
            Err(_) => { 
                self.log_error("[FUL] Could not acquire the lock to get last limits".to_string());
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

    pub fn freeze_spectrum(&self) {
        let mut frozen_list = match self.frozen_spectra.lock() {
            Ok(spectra) => spectra,
            Err(_) => { 
                self.log_error("[FFS] Could not acquire the lock to get frozen spectra".to_string());
                return ();
            }
        };

        let spectrum = match self.last_spectrum.lock() {
            Ok(spectrum) => spectrum,
            Err(_) => { 
                self.log_error("[FFS] Could not acquire the lock to get last spectrum".to_string());
                return ();
            }
        };

        match &*spectrum {
            Some(spectrum) => { 
                frozen_list.push(spectrum.clone());
                self.log_info("[FFS] Freezing spectrum".to_string());
            },
            None => self.log_war("[FFS] No spectrum to freeze".to_string())
        }
    }

    pub fn delete_frozen_spectrum(&self, id: usize) {
        let mut frozen_list = match self.frozen_spectra.lock() {
            Ok(spectra) => spectra,
            Err(_) => { 
                self.log_error("[FDF] Could not acquire the lock to get frozen spectra".to_string());
                return ();
            }
        };

        if id >= frozen_list.len() {
            self.log_error("[FDF] Could not delete the frozen spectrum, id out of bounds".to_string());
            return ();
        }

        frozen_list.remove(id);
        self.log_info(format!("[FDF] Deleting frozen {}", id));
    }

    pub fn get_frozen_spectrum_path(&self, id: usize, svg_limits: (u32, u32)) -> Option<String> {
        let frozen_list = match self.frozen_spectra.lock() {
            Ok(spectra) => spectra,
            Err(_) => { 
                self.log_error("[FGF] Could not acquire the lock to get frozen spectra".to_string());
                return None;
            }
        };

        if id >= frozen_list.len() {
            self.log_error("[FGF] Could not get the frozen spectrum, id out of bounds".to_string());
            return None;
        }

        let spectrum = &frozen_list[id];

        let spec_limits = match self.spectrum_limits.lock() {
            Ok(spec_limits) => spec_limits,
            Err(_) => { 
                self.log_error("[FGF] Could not acquire the lock to get last limits".to_string());
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
                self.log_error("[FCF] Could not acquire the lock to get frozen spectra".to_string());
                return None;
            }
        };

        if id >= frozen_list.len() {
            self.log_error("[FCF] Could not get the frozen spectrum, id out of bounds".to_string());
            return None;
        }

        let spectrum = &frozen_list[id];
        Some(spectrum.clone())
    }

    pub fn save_frozen(&self, id: usize, path: &Path) {
        let frozen_list = match self.frozen_spectra.lock() {
            Ok(spectra) => spectra,
            Err(_) => { 
                self.log_error("[FSF] Could not acquire the lock to get frozen spectra".to_string());
                return ();
            }
        };

        if id >= frozen_list.len() {
            self.log_error("[FSF] Could not get the frozen spectrum, id out of bounds".to_string());
            return ();
        }

        let spectrum = &frozen_list[id];

        match spectrum.save(path) {
            Ok(_) => self.log_info(format!("[FSF] Spectrum {} saved", id)),
            Err(error) => self.log_error(format!("[FSF] Failed to save spectrum {} ({})", id, error))
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

pub fn new_file_reader(path: String, log_sender: SyncSender<Log>) -> FileReader {
    FileReader {
        path,
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
    log_tx: Arc<SyncSender<Log>>
) -> Result<(), ()> {
    let event = match response {
        Ok(event) if event.kind.is_create() => event,
        Ok(_) => return Ok(()),                                // Don't care about successfull non create events
        Err(error) => {
            log_error(&log_tx, format!("[FWC] watch error: {:?}", error));
            return Err(());
        }
    };

    let text = match read_file_event(&event) {
        Ok(text) => text,
        Err(error) => {
            log_error(&log_tx, format!("[FWC] Could not react to the create \
                file event: {:?}, \nError: {}", event, error));
            return Err(());
        }
    };

    let spectrum = match Spectrum::from_str(&text) {
        Ok(spectrum) => spectrum,
        Err(error) => {
            log_error(&log_tx, format!("[FWC] Could not transform the file into \
                a spectrum ({})", error));
            return Err(());
        }
    };

    if saving.load(atomic::Ordering::Relaxed) {
        match auto_save_spectrum(&spectrum) {
            Ok(num) => log_info(&log_tx, format!("[FWC] Saved new spectrum {:03}", num)),
            Err(error) => log_error(&log_tx, format!("[FWC] Could not save new spectrum ({})", error))
        }
    }

    match last_spectrum.lock() {
        Ok(mut last_spectrum) => {
            *last_spectrum = Some(spectrum);
            new_spectrum.store(true, atomic::Ordering::Relaxed);
        },
        Err(error) => {
            log_error(&log_tx, format!("[FWC] Could not acquire the spectrum lock ({})", error));
            return Err(());
        }
    };

    Ok(())
}

fn auto_save_spectrum(spectrum: &Spectrum) -> Result<u32, Box<dyn Error>> {
    let folder_path = Path::new("C:\\Users\\guilh\\Desktop\\Coisas\\temp\\spectra");    // TODO send to config
    fs::create_dir_all(folder_path)?;

    for i in 0..100_000 {
        let new_path = folder_path.join(format!("spectrum{:03}.txt", i));
        if !new_path.exists() {
            spectrum.save(&new_path)?;
            return Ok(i);
        }
    } 

    Err(Box::new(io::Error::new(io::ErrorKind::Other, "Spectrum overflow,\
        can only save up to spectrum99999")))
}

