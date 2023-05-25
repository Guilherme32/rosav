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

use std::path::PathBuf;

use serde::{Serialize, Deserialize};

use crate::spectrum::*;
use crate::spectrum_backend::State;


// Region: Main declarations ---------------------------------------------------

#[derive(Debug)]
pub struct FileReaderConfig {
    pub watcher_path: PathBuf
}

#[derive(Debug)]
pub struct FileReader {
    pub state: Arc<Mutex<ReaderState>>,
    pub log_sender: SyncSender<Log>,
    pub config: Mutex<FileReaderConfig>
}


// Region Helper declarations --------------------------------------------------

#[derive(Debug)]
pub enum ReaderState {
    Disconnected,
    Connected,
    Reading (notify::RecommendedWatcher)
}

#[derive(Debug)]
pub enum FileReaderError {
    ReaderAlreadyConnected,
    ReaderAlreadyDisconnected,
    PathDoesNotExist,
    PathIsNotDir,
    PathWithoutPermission,
    ReaderAlreadyReading,
    NotifyInternalError,
    ReaderNotReading
}

// Region: required impls ------------------------------------------------------

impl FileReader {
    pub fn connect(&self) -> Result<(), ConnectError> {
        let mut state = self.state.lock().unwrap();

        match *state {
            ReaderState::Disconnected => (),
            _ => {
                self.log_war("[FCN] Não foi possível conectar: O aquisitor já \
                    está conectado".to_string());
                return Err(FileReaderError::ReaderAlreadyConnected);
            }
        }

        let config = self.config.lock().unwrap();
        let path = Path::new(&config.watcher_path);

        match path.try_exists() {
            Err(_) => { 
                self.log_war("[FCN] Não foi possível conectar: A permissão para
                    acessar o caminho configurado foi negada".to_string());
                return Err(FileReaderError::PathWithoutPermission);
            },
            Ok(exists) => {
                if !exists {
                    self.log_war("[FCN] Não foi possível conectar: O caminho \
                        configurado não existe".to_string());
                    return Err(FileReaderError::PathDoesNotExist);
                }
            }
        }

        if !path.is_dir() {
            self.log_war("[FCN] Não foi possível conectar: O caminho \
                configurado não é uma pasta".to_string());
            return Err(FileReaderError::PathIsNotDir);
        }

        *state = ReaderState::Connected;
        self.log_info("[FCN] Aquisitor conectado".to_string());
        Ok(())
    }

    pub fn disconnect(&self) -> Result<(), FileReaderError> {
        let mut state = self.state.lock().unwrap();

        match *state {
            ReaderState::Disconnected => {
                self.log_war("[FDN] Não foi possível desconectar: Aquisitor \
                    já está desconectado".to_string());
                return Err(FileReaderError::ReaderAlreadyConnected);
            },
            _ => ()
        }

        *state = ReaderState::Disconnected;
        self.log_info("[FDN] Aquisitor desconectado".to_string());
        Ok(())
    }

    pub fn start_reading(
        &self,
        handler: &SpectrumHandler
    ) -> Result<(), FileReaderError> {
        let mut state = self.state.lock().unwrap();

        match *state {
            ReaderState::Disconnected => {
                self.log_war("FSR Não foi possível começar a ler: Aquisitor \
                    está desconectado".to_string());
                return Err(FileReaderError::ReaderNotConnected);
            },
            ReaderState::Reading(_) => {
                self.log_war("FSR Não foi possível começar aler: Aquisitor \
                    já está lendo".to_string());
                return Err(FileReaderError::ReaderAlreadyReading);
            },
            _ => ()
        }

        let config = handler.config.lock().unwrap();

        let watcher_path = Path::new(&config.watcher_path);
        let auto_save_path = config.auto_save_path.clone();

        let spectrum_reference = Arc::clone(&handler.last_spectrum);
        let flag_reference = Arc::clone(&handler.unread_spectrum);
        let saving_reference = Arc::clone(&handler.saving_new);
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
                let state = state_reference.lock().unwrap();
                *state = ReaderState::Disconnected;
                log_war(&log_sender_clone, "[FSR] Aquisitor desconectado \
                    devido a um erro".to_string());
            }
        };

        let mut watcher = match notify::recommended_watcher(callback) {
            Ok(_watcher) => _watcher,
            Err(_) => return Err(FileReaderError::NotifyInternalError)
        };

        match watcher.watch(watcher_path, RecursiveMode::NonRecursive) {
            Ok(_) => (),
            Err(_) => return Err(FileReaderError::NotifyInternalError)
        }

        *state = ReaderState::Reading(watcher);
        self.log_info("[FSR] Aquisitor lendo".to_string());
        Ok(())
    }

    pub fn stop_reading(&self) -> Result<(), FileReaderError> {
        let mut state = self.state.lock().unwrap();

        match *state {
            ReaderState::Reading(_) => (),
            _ => {
                self.log_war("[FTP] Não foi possível parar de ler, o aquisitor \
                    não estava lendo".to_string());
                return Err(FileReaderError::ReaderNotReading);
            }
        }

        *state = ReaderState::Connected;
        self.log_info("[FTP] Aquisitor parou de ler".to_string());
        Ok(())
    }

    pub fn get_simplified_state(&self) -> State {
        let state = self.state.lock().unwrap();

        match *state {
            FileReaderState::Connected => State::Connected,
            FileReaderState::Disconnected => State::Disconnected,
            FileReaderState::Reading(_) => State::Reading
        }
    }
}


// Region: Outside impls -------------------------------------------------------

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
    auto_save_path: &Path,
    log_tx: Arc<SyncSender<Log>>
) -> Result<(), ()> {
    let event = match response {
        Ok(event) if event.kind.is_create() => event,
        Ok(_) => return Ok(()),                      // Don't care about successfull non create events
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

    let last_spectrum = last_spectrum.lock().unwrap();
    *last_spectrum = Some(spectrum);
    new_spectrum.store(true, atomic::Ordering::Relaxed);

    Ok(())
}





