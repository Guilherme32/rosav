use crate::{log_error, log_info, log_war, Log};

use notify;
use notify::{RecursiveMode, Watcher};
use std::error::Error;
use std::fs::{self, File};
use std::io;
use std::io::Read;
use std::path::Path;

use std::sync::atomic::{self, AtomicBool};
use std::sync::mpsc::SyncSender;
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::spectrum::*;
use crate::spectrum_handler::{SpectrumHandler, State};

// Region: Main declarations ---------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct FileReaderConfig {
    pub watcher_path: PathBuf,
}

#[derive(Debug)]
pub struct FileReader {
    pub state: Arc<Mutex<ReaderState>>,
    pub log_sender: SyncSender<Log>,
    pub config: Mutex<FileReaderConfig>,
}

// Region: Default generators --------------------------------------------------

pub fn new_file_reader(config: FileReaderConfig, log_sender: SyncSender<Log>) -> FileReader {
    FileReader {
        state: Arc::new(Mutex::new(ReaderState::Disconnected)),
        log_sender,
        config: Mutex::new(config),
    }
}

pub fn default_config() -> FileReaderConfig {
    FileReaderConfig {
        watcher_path: PathBuf::from("./"),
    }
}

// Region Helper declarations --------------------------------------------------

#[derive(Debug)]
pub enum ReaderState {
    Disconnected,
    Connected,
    Reading(notify::RecommendedWatcher),
}

#[derive(Debug, thiserror::Error)]
pub enum FileReaderError {
    #[error("Leitor já conectado")]
    ReaderAlreadyConnected,

    #[error("Leitor já desconectado")]
    ReaderAlreadyDisconnected,

    #[error("Caminho não existe")]
    PathDoesNotExist,

    #[error("Caminho não é pasta ")]
    PathIsNotDir,

    #[error("Permissão negada ao caminho")]
    PathWithoutPermission,

    #[error("Leitor já está lendo")]
    ReaderAlreadyReading,

    #[error("Erro interno da biblioteca Notify")]
    NotifyInternalError,

    #[error("Leitor não está conectado")]
    ReaderNotConnected,

    #[error("Leitor não está lendo")]
    ReaderNotReading,
}

// Region: required impls ------------------------------------------------------

impl FileReader {
    pub fn connect(&self) -> Result<(), FileReaderError> {
        let mut state = self.state.lock().unwrap();

        match *state {
            ReaderState::Disconnected => (),
            _ => {
                self.log_war(
                    "[FCN] Não foi possível conectar: O aquisitor já \
                    está conectado"
                        .to_string(),
                );
                return Err(FileReaderError::ReaderAlreadyConnected);
            }
        }

        let config = self.config.lock().unwrap();
        let path = Path::new(&config.watcher_path);

        match path.try_exists() {
            Err(_) => {
                self.log_war(
                    "[FCN] Não foi possível conectar: A permissão para
                    acessar o caminho configurado foi negada"
                        .to_string(),
                );
                return Err(FileReaderError::PathWithoutPermission);
            }
            Ok(exists) => {
                if !exists {
                    self.log_war(
                        "[FCN] Não foi possível conectar: O caminho \
                        configurado não existe"
                            .to_string(),
                    );
                    return Err(FileReaderError::PathDoesNotExist);
                }
            }
        }

        if !path.is_dir() {
            self.log_war(
                "[FCN] Não foi possível conectar: O caminho \
                configurado não é uma pasta"
                    .to_string(),
            );
            return Err(FileReaderError::PathIsNotDir);
        }

        *state = ReaderState::Connected;
        self.log_info("[FCN] Aquisitor conectado".to_string());
        Ok(())
    }

    pub fn disconnect(&self) -> Result<(), FileReaderError> {
        let mut state = self.state.lock().unwrap();

        if let ReaderState::Disconnected = *state {
            self.log_war(
                "[FDN] Não foi possível desconectar: Aquisitor \
                já está desconectado"
                    .to_string(),
            );
            return Err(FileReaderError::ReaderAlreadyConnected);
        }

        *state = ReaderState::Disconnected;
        self.log_info("[FDN] Aquisitor desconectado".to_string());
        Ok(())
    }

    pub fn start_reading(
        &self,
        handler: &SpectrumHandler,
        single_read: bool,
    ) -> Result<(), FileReaderError> {
        let mut state = self.state.lock().unwrap();

        match *state {
            ReaderState::Disconnected => {
                self.log_war(
                    "[FSR] Não foi possível começar a ler: Aquisitor \
                    está desconectado"
                        .to_string(),
                );
                return Err(FileReaderError::ReaderNotConnected);
            }
            ReaderState::Reading(_) => {
                self.log_war(
                    "[FSR] Não foi possível começar aler: Aquisitor \
                    já está lendo"
                        .to_string(),
                );
                return Err(FileReaderError::ReaderAlreadyReading);
            }
            _ => (),
        }

        let config = self.config.lock().unwrap();
        let handler_config = handler.config.lock().unwrap();

        let watcher_path = Path::new(&config.watcher_path);
        let auto_save_path = handler_config.auto_save_path.clone();

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
            Arc::clone(&log_sender_clone),
        ) {
            Ok(_) => {
                if single_read {
                    let mut state = state_reference.lock().unwrap();
                    *state = ReaderState::Connected;
                }
            }
            Err(_) => {
                let mut state = state_reference.lock().unwrap();
                *state = ReaderState::Disconnected;
                log_war(
                    &log_sender_clone,
                    "[FSR] Aquisitor desconectado \
                    devido a um erro"
                        .to_string(),
                );
            }
        };

        let mut watcher = match notify::recommended_watcher(callback) {
            Ok(_watcher) => _watcher,
            Err(_) => return Err(FileReaderError::NotifyInternalError),
        };

        match watcher.watch(watcher_path, RecursiveMode::NonRecursive) {
            Ok(_) => (),
            Err(_) => return Err(FileReaderError::NotifyInternalError),
        }

        *state = ReaderState::Reading(watcher);
        if single_read {
            self.log_info("[FSR] Aquisitor lendo 1 espectro".to_string());
        } else {
            self.log_info("[FSR] Aquisitor lendo contínuo".to_string());
        }
        Ok(())
    }

    pub fn stop_reading(&self) -> Result<(), FileReaderError> {
        let mut state = self.state.lock().unwrap();

        match *state {
            ReaderState::Reading(_) => (),
            _ => {
                self.log_war(
                    "[FTP] Não foi possível parar de ler, o aquisitor \
                    não estava lendo"
                        .to_string(),
                );
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
            ReaderState::Connected => State::Connected,
            ReaderState::Disconnected => State::Disconnected,
            ReaderState::Reading(_) => State::Reading,
        }
    }
}

//Region: Config ---------------------------------------------------------------

impl FileReader {
    pub fn update_config(&self, new_config: FileReaderConfig) {
        let mut config = self.config.lock().unwrap();

        *config = new_config;
    }

    pub fn get_config(&self) -> FileReaderConfig {
        let config = self.config.lock().unwrap();

        (*config).clone()
    }
}

// Region: Loggers -------------------------------------------------------------

impl FileReader {
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

// Region: Outside impls -------------------------------------------------------

fn read_file_event(event: &notify::Event) -> Result<String, Box<dyn Error>> {
    let path = &event.paths[0];

    for _ in 0..10 {
        let mut file = match File::open(path) {
            // Tries to open the file
            Ok(_file) => _file, // Will retry 10 times if
            Err(err) if err.raw_os_error() == Some(32) => {
                // it can't open because someone
                sleep(Duration::from_millis(100)); // else is using it (os err 32)
                continue;
            }
            Err(err) => {
                return Err(Box::new(err));
            }
        };

        let mut contents = String::new();
        file.read_to_string(&mut contents)?;

        return Ok(contents);
    }

    // The only way it gets here is if error 32 happened 10 times in a row
    Err(Box::new(io::Error::from_raw_os_error(32)))
}

fn auto_save_spectrum(spectrum: &Spectrum, folder_path: &Path) -> Result<u32, Box<dyn Error>> {
    fs::create_dir_all(folder_path)?;

    for i in 0..100_000 {
        let new_path = folder_path.join(format!("spectrum{:03}.txt", i));
        if !new_path.exists() {
            spectrum.save(&new_path)?;
            return Ok(i);
        }
    }

    Err(Box::new(io::Error::new(
        io::ErrorKind::Other,
        "Overflow de espectros,\
        o programa só suporta até 'spectrum99999'",
    )))
}

fn watcher_callback<T: std::fmt::Debug>(
    response: Result<notify::Event, T>,
    last_spectrum: Arc<Mutex<Option<Spectrum>>>,
    new_spectrum: Arc<AtomicBool>,
    saving: Arc<AtomicBool>,
    auto_save_path: &Path,
    log_tx: Arc<SyncSender<Log>>,
) -> Result<(), ()> {
    let event = match response {
        Ok(event) if event.kind.is_create() => event,
        Ok(_) => return Ok(()), // Don't care about successfull non create events
        Err(error) => {
            log_error(&log_tx, format!("[FWC] Erro do 'watcher': {:?}", error));
            return Err(());
        }
    };

    let text = match read_file_event(&event) {
        Ok(text) => text,
        Err(error) => {
            log_error(
                &log_tx,
                format!(
                    "[FWC] Não foi possível responder ao \
                'file event': {:?}, \nErro: {}",
                    event, error
                ),
            );
            return Err(());
        }
    };

    if text.is_empty() {
        // When editing files it reads as an empty string
        return Ok(()); // We can ignore that
    }

    let spectrum = match Spectrum::from_csv_text(&text) {
        Ok(spectrum) => spectrum,
        Err(error) => {
            log_error(
                &log_tx,
                format!(
                    "[FWC] Não foi posível transformar o \
                arquivo em um espectro ({})",
                    error
                ),
            );
            return Err(());
        }
    };

    if saving.load(atomic::Ordering::Relaxed) {
        match auto_save_spectrum(&spectrum, auto_save_path) {
            Ok(num) => log_info(&log_tx, format!("[FWC] Espectro {:03} salvo", num)),
            Err(error) => log_error(
                &log_tx,
                format!(
                    "[FWC] Não foi possível \
                salvar o espectro novo ({})",
                    error
                ),
            ),
        }
    }

    let mut last_spectrum = last_spectrum.lock().unwrap();
    *last_spectrum = Some(spectrum);
    new_spectrum.store(true, atomic::Ordering::Relaxed);

    Ok(())
}
