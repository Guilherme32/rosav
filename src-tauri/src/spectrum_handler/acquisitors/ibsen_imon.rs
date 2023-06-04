use crate::{ Log, log_info, log_error, log_war };

use std::path::Path;
use std::fs;
use std::io::Read;
use std::io;
use std::error::Error;

use std::time::Duration;
use std::thread::{ self, sleep };
use std::sync::{Arc, Mutex};
use std::sync::atomic::{ AtomicBool, Ordering };
use std::sync::mpsc::{ self, Receiver, SyncSender, TryRecvError };

use serde::{Serialize, Deserialize};

use serialport::{
    available_ports,
    new,
    SerialPort,
    ClearBuffer,
    SerialPortType::UsbPort,
    SerialPortInfo
};

use crate::spectrum::*;
use crate::spectrum_handler::{ State, SpectrumHandler };

// TODO use a trait to make the integration of new acquistors easier

// Region: Main declarations ---------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ImonConfig {
    pub multisampling: u64,
    pub exposure_ms: u64,
    pub read_delay_ms: u64
}

#[derive(Debug)]
pub struct Imon {
    state: Arc<Mutex<ImonState>>,
    pub log_sender: SyncSender<Log>,
    pub config: Mutex<ImonConfig>
}


// Region: Default generators --------------------------------------------------

pub fn new_imon(
    config: ImonConfig,
    log_sender: SyncSender<Log>
) -> Imon {
    Imon {
        state: Arc::new(Mutex::new(ImonState::Disconnected)),
        log_sender,
        config: Mutex::new(config)
    }
}

pub fn default_config() -> ImonConfig {
    ImonConfig {
        multisampling: 1,
        exposure_ms: 10,
        read_delay_ms: 100
    }
}

// Region Helper declarations --------------------------------------------------

#[derive(Debug)]
enum ImonState {
    Disconnected,
    Connected(ConnectedImon),
    Reading(ReadingImon)
}

#[derive(Debug, Clone)]
struct ConnectedImon {
    port: Arc<Mutex<Box<dyn SerialPort>>>,
    n_pixels: u32,
    pixel_fit_coefficients: Vec<f64>
}

#[derive(Debug)]
struct ReadingImon {
    connected_imon: ConnectedImon,
    config_tx: mpsc::Sender<ImonConfig>
}

#[derive(Debug, thiserror::Error)]
pub enum ImonError {
    #[error("Imon já conectado")]
    ImonAlreadyConnected,

    #[error("Imon já desconectado")]
    ImonAlreadyDisconnected,

    #[error("Caminho não existe")]
    PathDoesNotExist,

    #[error("Caminho não é pasta ")]
    PathIsNotDir,

    #[error("Permissão negada ao caminho")]
    PathWithoutPermission,

    #[error("Imon já está lendo")]
    ImonAlreadyReading,

    #[error("Imon não está conectado")]
    ImonNotConnected,

    #[error("Imon não está lendo")]
    ImonNotReading,

    #[error("Dispositivo serial não é o imon (ou não respondeu)")]
    NotImon,

    #[error("Imon não encontrado nos dispositivos serial conectados")]
    ImonNotFound,

    #[error("Encontrou o Imon, mas não conseguiu pegar os dados")]
    ParseError,

    #[error("Comando não entendido pelo IMON")]
    CommandNack,

    #[error("Comando não foi respondido da forma esperada")]
    UnexpectedResponse
}

// Region: required impls ------------------------------------------------------

impl Imon {
    pub fn connect(&self) -> Result<(), Box<dyn Error>> {
        let mut state = self.state.lock().unwrap();

        match &*state {
            ImonState::Disconnected => (),
            _ => {
                self.log_war("[FCN] Não foi possível conectar: O aquisitor já \
                    está conectado".to_string());
                return Err(Box::new(ImonError::ImonAlreadyConnected));
            }
        }

        let port = match find_imon() {
            Ok(found) => found,
            Err(err) => {
                self.log_war(format!("[ICN] Não foi possível conectar. IMON não encontrado
                    ({})", err));
                return Err(err);
            }
        };

        let connected_imon = match parse_imon_parameters(port) {
            Ok(parsed) => parsed,
            Err(err) => {
                self.log_war(format!("[ICN] Não foi possível conectar. Falha na extração
                    dos parâmetros do IMON ({})", err));
                return Err(err);
            }
        };

        *state = ImonState::Connected(connected_imon);
        self.log_info("[ICN] Aquisitor conectado".to_string());
        Ok(())
    }

    pub fn disconnect(&self) -> Result<(), ImonError> {
        let mut state = self.state.lock().unwrap();

        match &*state {
            ImonState::Disconnected => {
                self.log_war("[IDN] Não foi possível desconectar: Aquisitor \
                    já está desconectado".to_string());
                return Err(ImonError::ImonAlreadyConnected);
            },
            _ => ()
        }

        *state = ImonState::Disconnected;
        self.log_info("[IDN] Aquisitor desconectado".to_string());
        Ok(())
    }

    pub fn start_reading(
        &self,
        handler: &SpectrumHandler
    ) -> Result<(), ImonError> {
        let mut state = self.state.lock().unwrap();

        match &mut *state {
            ImonState::Disconnected => {
                self.log_war("[ISR] Não foi possível começar a ler: Aquisitor \
                    está desconectado".to_string());
                return Err(ImonError::ImonNotConnected);
            },
            ImonState::Reading(_) => {
                self.log_war("[ISR] Não foi possível começar a ler: Aquisitor \
                    já está lendo".to_string());
                return Err(ImonError::ImonAlreadyReading);
            },
            ImonState::Connected(connected_imon) => {
                let current_config = self.config.lock().unwrap();

                let (config_tx, config_rx) = mpsc::channel();
                let _ = config_tx.send((*current_config).clone());

                let log_sender_clone = Arc::new(self.log_sender.clone());

                let handler_config = handler.config.lock().unwrap();
                let auto_save_path = handler_config.auto_save_path.clone();

                let spectrum_reference = Arc::clone(&handler.last_spectrum);
                let flag_reference = Arc::clone(&handler.unread_spectrum);
                let saving_reference = Arc::clone(&handler.saving_new);
                let state_reference = Arc::clone(&self.state);

                let port_reference = Arc::clone(&connected_imon.port);
                let n_pixels = connected_imon.n_pixels;
                let pixel_fit_coefs_clone = connected_imon.pixel_fit_coefficients.clone();

                thread::spawn(move || {
                    constant_read(
                        spectrum_reference,
                        flag_reference,
                        saving_reference,
                        &auto_save_path,
                        log_sender_clone,
                        state_reference,
                        config_rx,
                        port_reference,
                        n_pixels,
                        pixel_fit_coefs_clone
                    );
                });

                *state = ImonState::Reading(ReadingImon{
                    connected_imon: connected_imon.clone(),
                    config_tx
                });
            }
        }

        Ok(())
    }

    pub fn stop_reading(&self) -> Result<(), ImonError> {
        let mut state = self.state.lock().unwrap();

        match &*state {
            ImonState::Reading(reading_imon) => {
                *state = ImonState::Connected(reading_imon.connected_imon.clone());
                self.log_info("[ITP] Aquisitor parou de ler".to_string());
                Ok(())
            },
            _ => {
                self.log_war("[ITP] Não foi possível parar de ler, o aquisitor \
                    não estava lendo".to_string());
                Err(ImonError::ImonNotReading)
            }
        }
    }

    pub fn get_simplified_state(&self) -> State {
        let state = self.state.lock().unwrap();

        match &*state {
            ImonState::Connected(_) => State::Connected,
            ImonState::Disconnected => State::Disconnected,
            ImonState::Reading(_) => State::Reading
        }
    }
}


//Region: Config ---------------------------------------------------------------

impl Imon {
    pub fn update_config(&self, new_config: ImonConfig) {
        let mut config = self.config.lock().unwrap();
        let state = self.state.lock().unwrap();
        if let ImonState::Reading(reading_imon) = &*state {
            let _ = reading_imon.config_tx.send(config.clone());
        }

        *config = new_config;
    }

    pub fn get_config(&self) -> ImonConfig {
        let config = self.config.lock().unwrap();

        (*config).clone()
    }
}

// Region: Loggers -------------------------------------------------------------

impl Imon {
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

fn find_imon() -> Result<Box<dyn SerialPort>, Box<dyn Error>> {
    let ports = available_ports()?;

    for port in ports {
        if let Ok(port) = is_imon(port) {
            return Ok(port);
        }
    }

    Err(Box::new(ImonError::ImonNotFound))
}

fn is_imon(port: SerialPortInfo) -> Result<Box<dyn SerialPort>, Box<dyn Error>> {
     match port.port_type {
        UsbPort(_) => {
            let mut port = new(port.port_name, 921_000)
                .timeout(Duration::from_millis(5))
                .open()?;

            port.clear(ClearBuffer::Input)?;
            port.write(b"*IDN\r")?;
        
            let mut buffer: [u8; 1024] = [0;1024];
            port.read(&mut buffer)?;
            let response = String::from_utf8_lossy(&buffer);

            // TODO check if ID matches here
            if response.len() != 0 {
                println!("Resposta do *IDN: \n{}", response);            // TODO remove
                return Ok(port);
            }
        },
        _ => ()
    }   

    Err(Box::new(ImonError::NotImon))
}

fn parse_imon_parameters(
    mut port: Box<dyn SerialPort>
) -> Result<ConnectedImon, Box<dyn Error>> {
    port.clear(ClearBuffer::Input)?;
    port.write(b"*para:basic?\r")?;

    let mut buffer: [u8; 4096] = [0; 4096];
    port.read(&mut buffer)?;
    let response = String::from_utf8_lossy(&buffer);

    println!("Response from *para:basic: \n{}", response);            // TODO remove

    let mut n_pixels: Option<u32> = None;
    let mut fit_coefficients: Vec<f64> = vec![];

    for line in response.split('\r') {
        let line = line.replace('\n', "");
        let line = line.replace(' ', "");
        let line = line.to_lowercase();

        if let Some(_n_pixels) = line.strip_prefix("pixelperline:") {
            if let Ok(_n_pixels) = _n_pixels.parse() {
                n_pixels = Some(_n_pixels);
            }
        }

        let coef_index = fit_coefficients.len();
        let expected_fit_coef = format!("channel0fitx^{}:", coef_index);

        if let Some(coef) = line.strip_prefix(&expected_fit_coef) {
            if let Ok(coef) = coef.parse::<f64>() {
                fit_coefficients.push(coef * 1e-9);
            }
        }
    }

    if let Some(n_pixels) = n_pixels {
        if fit_coefficients.len() >= 5 {
            return Ok(ConnectedImon {
                port: Arc::new(Mutex::new(port)),
                n_pixels,
                pixel_fit_coefficients: fit_coefficients
            })
        }
    }

    Err(Box::new(ImonError::ParseError))
}

fn constant_read(
    last_spectrum: Arc<Mutex<Option<Spectrum>>>,
    new_spectrum: Arc<AtomicBool>,
    saving: Arc<AtomicBool>,
    auto_save_path: &Path,
    log_tx: Arc<SyncSender<Log>>,
    state: Arc<Mutex<ImonState>>,
    config_rx: Receiver<ImonConfig>,
    port: Arc<Mutex<Box<dyn SerialPort>>>,
    n_pixels: u32,
    pixel_fit_coefficients: Vec<f64>,
) {
    let mut config = default_config();
    loop {
        sleep(Duration::from_millis(config.read_delay_ms));

        let mut port = port.lock().unwrap();

        match config_rx.try_recv() {
            Ok(new_config) => config = new_config,
            Err(TryRecvError::Empty) => (),
            Err(TryRecvError::Disconnected) => break
        }

        match get_spectrum(
            &mut *port,
            &config,
            n_pixels,
            &pixel_fit_coefficients
        ) {
            Ok(spectrum) => {
                if saving.load(Ordering::Relaxed) {
                    let _ = auto_save_spectrum(&spectrum, &auto_save_path);
                }
                let mut last_spectrum = last_spectrum.lock().unwrap();
                *last_spectrum = Some(spectrum);
                new_spectrum.store(true, Ordering::Relaxed);
            },
            Err(error) => {
                log_error(&log_tx, format!("[IRS] Erro na acquisição \
                    do espectro: {}", error));
                log_war(&log_tx, format!("[IRS] Aquisitor desconectado devido \
                    a um erro"));
                let mut state = state.lock().unwrap();
                *state = ImonState::Disconnected;
                break;
            }
        }
    }
    
}

fn get_spectrum(
    port: &mut Box<dyn SerialPort>,
    config: &ImonConfig,
    n_pixels: u32,
    pixel_fit_coefficients: &Vec<f64>
) -> Result<Spectrum, Box<dyn Error>> {
    let command = format!(
        "*meas {} {} 3\r",            // *meas tint av format<CR>
        config.exposure_ms,
        config.multisampling
    ).into_bytes();

    port.clear(ClearBuffer::Input)?;
    port.write(&command)?;

    let mut buffer_single: [u8; 1] = [0; 1];
    port.read_exact(&mut buffer_single)?;

    if buffer_single[0] == 0x15 {
        return Err(Box::new(ImonError::CommandNack));
    }
    if buffer_single[0] != 0x07 {
        return Err(Box::new(ImonError::UnexpectedResponse));
    }

    sleep(Duration::from_millis(config.multisampling*config.exposure_ms));
    port.read_exact(&mut buffer_single)?;

    if buffer_single[0] != 0x07 {
        return Err(Box::new(ImonError::UnexpectedResponse));
    }

    let mut buffer_two: [u8; 2] = [0; 2];
    port.read_exact(&mut buffer_two)?;

    let length: u32 = buffer_two[0] as u32 + (buffer_two[1] as u32) << 8;

    let mut bit_sum: u32 = 0;
    let mut pixel_readings: Vec::<u32> = Vec::new();

    for _ in 0..n_pixels {
        port.read_exact(&mut buffer_two)?;
        let reading: u32 = buffer_two[0] as u32 + (buffer_two[1] as u32) << 8;

        pixel_readings.push(reading);
        bit_sum += reading.count_ones();
    }

    port.read_exact(&mut buffer_two)?;
    let checksum: u32 = buffer_two[0] as u32 + (buffer_two[1] as u32) << 8;

    println!("length: {}", length);
    println!("bit_sum: {}", bit_sum);
    println!("checksum: {}", checksum);            // TODO remove after testing

    Ok(Spectrum::from_ibsen_imon(&pixel_readings, pixel_fit_coefficients))
}

fn auto_save_spectrum(
    spectrum: &Spectrum,
    folder_path: &Path
) -> Result<u32, Box<dyn Error>> {
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

