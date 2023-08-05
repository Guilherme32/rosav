use crate::{log_error, log_info, log_war, Log};

use std::error::Error;
use std::fs;
use std::io;
use std::io::Read;

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{self, Receiver, SyncSender, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread::{self, sleep};
use std::time::Duration;

use serde::{Deserialize, Serialize};

use serialport::{
    available_ports, new, ClearBuffer, SerialPort, SerialPortInfo, SerialPortType::UsbPort,
};

use crate::spectrum::*;
use crate::spectrum_handler::{SpectrumHandler, State};

// TODO use a trait to make the integration of new acquistors easier

// TODO refatorar o código pra deixar mais bunitinho

// Region: Main declarations ---------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ImonConfig {
    pub exposure_ms: f64,
    pub read_delay_ms: u64,
}

#[derive(Debug)]
pub struct Imon {
    state: Arc<Mutex<ImonState>>,
    pub log_sender: SyncSender<Log>,
    pub config: Mutex<ImonConfig>,
}

// Region: Default generators --------------------------------------------------

pub fn new_imon(config: ImonConfig, log_sender: SyncSender<Log>) -> Imon {
    Imon {
        state: Arc::new(Mutex::new(ImonState::Disconnected)),
        log_sender,
        config: Mutex::new(config),
    }
}

pub fn default_config() -> ImonConfig {
    ImonConfig {
        exposure_ms: 0.01,
        read_delay_ms: 100,
    }
}

// Region Helper declarations --------------------------------------------------

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct ImonCoefficients {
    pub wavelength: [f64; 6],
    pub temperature: [f64; 4],
}

#[derive(Debug)]
enum ImonState {
    Disconnected,
    Connected(ConnectedImon),
    Reading(ReadingImon),
}

#[derive(Debug, Clone)]
struct ConnectedImon {
    port: Arc<Mutex<Box<dyn SerialPort>>>,
    n_pixels: u32,
    coefficients: ImonCoefficients,
    dark_pixels: Vec<u32>,
}

#[derive(Debug)]
struct ReadingImon {
    connected_imon: ConnectedImon,
    config_tx: mpsc::Sender<ImonConfig>,
}

#[derive(Debug, thiserror::Error)]
pub enum ImonError {
    #[error("Comando não entendido pelo IMON")]
    CommandNack,

    #[error("Imon já conectado")]
    ImonAlreadyConnected,

    #[error("Imon já desconectado")]
    ImonAlreadyDisconnected,

    #[error("Imon já está lendo")]
    ImonAlreadyReading,

    #[error("Imon não está conectado")]
    ImonNotConnected,

    #[error("Imon não encontrado nos dispositivos serial conectados")]
    ImonNotFound,

    #[error("Imon não está lendo")]
    ImonNotReading,

    #[error("Dispositivo serial não é o imon (ou não respondeu)")]
    NotImon,

    #[error("Encontrou o Imon, mas não conseguiu pegar os dados")]
    ParseError,

    #[error("Caminho não existe")]
    PathDoesNotExist,

    #[error("Caminho não é pasta ")]
    PathIsNotDir,

    #[error("Permissão negada ao caminho")]
    PathWithoutPermission,

    #[error("A fonte parece estar ligada. Ela deve estar desconectada para conectar o IMON")]
    SourceTurnedOn,

    #[error("Comando não foi respondido da forma esperada")]
    UnexpectedResponse,
}

// Region: required impls ------------------------------------------------------

impl Imon {
    pub fn connect(&self) -> Result<(), Box<dyn Error>> {
        let mut state = self.state.lock().unwrap();
        let config = self.config.lock().unwrap();

        match &*state {
            ImonState::Disconnected => (),
            _ => {
                self.log_war(
                    "[FCN] Não foi possível conectar: O aquisitor já \
                    está conectado"
                        .to_string(),
                );
                return Err(Box::new(ImonError::ImonAlreadyConnected));
            }
        }

        let port = match find_imon() {
            Ok(found) => found,
            Err(err) => {
                self.log_war(format!(
                    "[ICN] Não foi possível conectar. IMON não encontrado
                    ({})",
                    err
                ));
                return Err(err);
            }
        };

        let connected_imon = match parse_imon_parameters(port, &config) {
            Ok(parsed) => parsed,
            Err(err) => {
                self.log_war(format!(
                    "[ICN] Não foi possível conectar. Falha na extração
                    dos parâmetros do IMON ({})",
                    err
                ));
                return Err(err);
            }
        };

        *state = ImonState::Connected(connected_imon);
        self.log_info("[ICN] Aquisitor conectado".to_string());
        Ok(())
    }

    pub fn disconnect(&self) -> Result<(), ImonError> {
        let mut state = self.state.lock().unwrap();

        if let ImonState::Disconnected = &*state {
            self.log_war(
                "[IDN] Não foi possível desconectar: Aquisitor \
                já está desconectado"
                    .to_string(),
            );
            return Err(ImonError::ImonAlreadyConnected);
        }

        *state = ImonState::Disconnected;
        self.log_info("[IDN] Aquisitor desconectado".to_string());
        Ok(())
    }

    pub fn start_reading(&self, handler: &SpectrumHandler) -> Result<(), ImonError> {
        let mut state = self.state.lock().unwrap();

        match &mut *state {
            ImonState::Disconnected => {
                self.log_war(
                    "[ISR] Não foi possível começar a ler: Aquisitor \
                    está desconectado"
                        .to_string(),
                );
                return Err(ImonError::ImonNotConnected);
            }
            ImonState::Reading(_) => {
                self.log_war(
                    "[ISR] Não foi possível começar a ler: Aquisitor \
                    já está lendo"
                        .to_string(),
                );
                return Err(ImonError::ImonAlreadyReading);
            }
            ImonState::Connected(connected_imon) => {
                let current_config = self.config.lock().unwrap();

                let (config_tx, config_rx) = mpsc::channel();
                let _ = config_tx.send((*current_config).clone());

                let handler_config = handler.config.lock().unwrap();

                let constant_read_args = ConstantReadArgs {
                    last_spectrum: Arc::clone(&handler.last_spectrum),
                    new_spectrum_flag: Arc::clone(&handler.unread_spectrum),
                    saving: Arc::clone(&handler.saving_new),
                    auto_save_path: handler_config.auto_save_path.clone(),
                    log_tx: Arc::new(self.log_sender.clone()),
                    state: Arc::clone(&self.state),
                    config_rx,
                    port: Arc::clone(&connected_imon.port),
                    n_pixels: connected_imon.n_pixels,
                    coefficients: connected_imon.coefficients,
                    dark_pixels: connected_imon.dark_pixels.clone(),
                };

                thread::spawn(move || {
                    constant_read(constant_read_args);
                });

                *state = ImonState::Reading(ReadingImon {
                    connected_imon: connected_imon.clone(),
                    config_tx,
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
            }
            _ => {
                self.log_war(
                    "[ITP] Não foi possível parar de ler, o aquisitor \
                    não estava lendo"
                        .to_string(),
                );
                Err(ImonError::ImonNotReading)
            }
        }
    }

    pub fn get_simplified_state(&self) -> State {
        let state = self.state.lock().unwrap();

        match &*state {
            ImonState::Connected(_) => State::Connected,
            ImonState::Disconnected => State::Disconnected,
            ImonState::Reading(_) => State::Reading,
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
    if let UsbPort(_) = port.port_type {
        let mut port = new(port.port_name, 115200) //921_000)
            .timeout(Duration::from_millis(100))
            .open()?;

        port.clear(ClearBuffer::Input)?;
        port.write_all(b"*IDN?\r")?;

        let mut buffer: [u8; 1024] = [0; 1024];
        let _bytes_read = port.read(&mut buffer)?;
        let response = String::from_utf8_lossy(&buffer);

        if response.contains("JETI_VersaPIC_RU60") {
            return Ok(port);
        }
    }

    Err(Box::new(ImonError::NotImon))
}

fn parse_imon_parameters(
    mut port: Box<dyn SerialPort>,
    config: &ImonConfig,
) -> Result<ConnectedImon, Box<dyn Error>> {
    port.clear(ClearBuffer::Input)?;
    port.write_all(b"*para:basic?\r")?;

    let mut buffer: [u8; 4096] = [0; 4096];
    let _bytes_read = port.read(&mut buffer)?;
    let response = String::from_utf8_lossy(&buffer);

    let mut n_pixels: Option<u32> = None;

    for line in response.split('\r') {
        let line = line.replace('\n', "");
        let line = line.replace(' ', "");
        let line = line.to_lowercase();

        if let Some(_n_pixels) = line.strip_prefix("pixelperline:") {
            if let Ok(_n_pixels) = _n_pixels.parse() {
                n_pixels = Some(_n_pixels);
            }
        }
    }

    let coefficients = fetch_coefficients(&mut port)?;

    if let Some(n_pixels) = n_pixels {
        sleep(Duration::from_millis(10));

        let dark_pixels = get_raw_pixel_readings(&mut port, config, n_pixels)?;
        if dark_pixels.len() != n_pixels as usize {
            return Err(Box::new(ImonError::ParseError));
        }

        // Unwrap explanation: Length checked above
        if dark_pixels.iter().filter(|x| **x != 0).max().unwrap()
            - dark_pixels.iter().min().unwrap()
            > 500
        {
            return Err(Box::new(ImonError::SourceTurnedOn));
        }

        return Ok(ConnectedImon {
            port: Arc::new(Mutex::new(port)),
            n_pixels,
            coefficients,
            dark_pixels,
        });
    }

    Err(Box::new(ImonError::ParseError))
}

fn fetch_coefficients(port: &mut Box<dyn SerialPort>) -> Result<ImonCoefficients, Box<dyn Error>> {
    let mut coefficients = ImonCoefficients {
        wavelength: [0.0; 6],
        temperature: [0.0; 4],
    };

    // Get wavelength coefficients --------------------------------------------
    port.clear(ClearBuffer::Input)?;
    port.write_all(b"*rdusr2 0\r")?; // Read user flash memory block 0

    for i in 0..6 {
        let mut buffer: [u8; 16] = [0; 16];
        port.read_exact(&mut buffer)?;
        let buffer = String::from_utf8_lossy(&buffer);

        coefficients.wavelength[i] = buffer.parse()?;
    }

    sleep(Duration::from_millis(20)); // Without this the next step gets wrong values

    // Get temperature coefficients -------------------------------------------
    port.clear(ClearBuffer::Input)?;
    port.write_all(b"*rdusr2 1\r")?; // Read user flash memory block 1

    for i in 0..4 {
        let mut buffer: [u8; 16] = [0; 16];
        port.read_exact(&mut buffer)?;
        let buffer = String::from_utf8_lossy(&buffer);

        coefficients.temperature[i] = buffer.parse()?;
    }

    Ok(coefficients)
}

struct ConstantReadArgs {
    last_spectrum: Arc<Mutex<Option<Spectrum>>>,
    new_spectrum_flag: Arc<AtomicBool>,
    saving: Arc<AtomicBool>,
    auto_save_path: PathBuf,
    log_tx: Arc<SyncSender<Log>>,
    state: Arc<Mutex<ImonState>>,
    config_rx: Receiver<ImonConfig>,
    port: Arc<Mutex<Box<dyn SerialPort>>>,
    n_pixels: u32,
    coefficients: ImonCoefficients,
    dark_pixels: Vec<u32>,
}

fn constant_read(
    // last_spectrum: Arc<Mutex<Option<Spectrum>>>,
    // new_spectrum: Arc<AtomicBool>,
    // saving: Arc<AtomicBool>,
    // auto_save_path: &Path,
    // log_tx: Arc<SyncSender<Log>>,
    // state: Arc<Mutex<ImonState>>,
    // config_rx: Receiver<ImonConfig>,
    // port: Arc<Mutex<Box<dyn SerialPort>>>,
    // n_pixels: u32,
    // coefficients: ImonCoefficients,
    // dark_pixels: Vec<u32>,
    args: ConstantReadArgs,
) {
    let mut config = default_config();
    loop {
        sleep(Duration::from_millis(config.read_delay_ms));

        let mut port = args.port.lock().unwrap();

        match args.config_rx.try_recv() {
            Ok(new_config) => {
                config = new_config;
            }
            Err(TryRecvError::Empty) => (),
            Err(TryRecvError::Disconnected) => break,
        }

        for i in 0..10 {
            // Tries to get the spectrum 10 times
            match get_spectrum(
                &mut port,
                &config,
                args.n_pixels,
                &args.coefficients,
                &args.dark_pixels,
            ) {
                Ok(spectrum) => {
                    if args.saving.load(Ordering::Relaxed) {
                        let _ = auto_save_spectrum(&spectrum, &args.auto_save_path);
                    }
                    let mut last_spectrum = args.last_spectrum.lock().unwrap();
                    *last_spectrum = Some(spectrum);
                    args.new_spectrum_flag.store(true, Ordering::Relaxed);

                    break;
                }
                Err(error) => {
                    log_error(
                        &args.log_tx,
                        format!(
                            "[IRS] {}/10 Erro na acquisição \
                        do espectro: {}",
                            i + 1,
                            error
                        ),
                    );

                    if i == 9 {
                        log_war(
                            &args.log_tx,
                            "[IRS] Aquisitor desconectado devido a um erro".to_string(),
                        );
                        let mut state = args.state.lock().unwrap();
                        *state = ImonState::Disconnected;

                        return;
                    }
                }
            }
        }
    }
}

pub fn get_raw_pixel_readings(
    port: &mut Box<dyn SerialPort>,
    config: &ImonConfig,
    n_pixels: u32,
) -> Result<Vec<u32>, Box<dyn Error>> {
    let command = format!(
        "*meas {:.3} 1 3\r", // *meas tint (ms) av format<CR>
        config.exposure_ms
    )
    .into_bytes();

    port.clear(ClearBuffer::Input)?;
    port.write_all(&command)?;

    let mut buffer_single: [u8; 1] = [0; 1];

    check_ack(port)?;

    sleep(Duration::from_millis((config.exposure_ms) as u64 + 1));

    'check_bell: {
        // Searches for the bell (reading complete)
        for _ in 0..1000 {
            port.read_exact(&mut buffer_single)?;

            if buffer_single[0] == 0x07 {
                // Found nack
                break 'check_bell;
            }
        }

        return Err(Box::new(ImonError::UnexpectedResponse)); // Did not find it
    }

    let mut buffer_two: [u8; 2] = [0; 2];
    port.read_exact(&mut buffer_two)?;

    let _length: u32 = (buffer_two[0] as u32) + ((buffer_two[1] as u32) << 8);

    let mut pixel_readings: Vec<u32> = Vec::new();

    // TODO colocar length/2 aqui e remover n_pixels do codigo
    for _ in 0..n_pixels {
        port.read_exact(&mut buffer_two)?;
        let reading: u32 = (buffer_two[0] as u32) + ((buffer_two[1] as u32) << 8);

        pixel_readings.push(reading);
    }

    port.read_exact(&mut buffer_two)?;
    // TODO find out how this checksum works
    let _checksum: u32 = (buffer_two[0] as u32) + ((buffer_two[1] as u32) << 8);

    Ok(pixel_readings)
}

fn get_spectrum(
    port: &mut Box<dyn SerialPort>,
    config: &ImonConfig,
    n_pixels: u32,
    coefficients: &ImonCoefficients,
    dark_pixels: &[u32],
) -> Result<Spectrum, Box<dyn Error>> {
    let pixel_readings = get_raw_pixel_readings(port, config, n_pixels)?;

    let temperature = get_temperature(port).unwrap_or(25.314);

    Ok(Spectrum::from_ibsen_imon(
        &pixel_readings,
        temperature,
        coefficients,
        dark_pixels,
    ))
}

fn get_temperature(port: &mut Box<dyn SerialPort>) -> Result<f64, Box<dyn Error>> {
    let command = b"*meas:tempe\r";

    port.clear(ClearBuffer::Input)?;
    port.write_all(command)?;

    let mut buffer: [u8; 64] = [0; 64];
    let _bytes_read = port.read(&mut buffer)?;
    let response = String::from_utf8_lossy(&buffer);

    for line in response.split('\r') {
        let line = line.replace(' ', "");
        let line = line.replace('\t', ""); // Tabs
        let line = line.replace('\n', "");

        for word in line.split(':') {
            if let Ok(temperature) = word.parse() {
                return Ok(temperature);
            }
        }
    }

    Err(Box::new(ImonError::UnexpectedResponse))
}

fn check_ack(port: &mut Box<dyn SerialPort>) -> Result<(), Box<dyn Error>> {
    let mut buffer_single: [u8; 1] = [0; 1];

    for _ in 0..100 {
        port.read_exact(&mut buffer_single)?;

        if buffer_single[0] == 0x15 {
            // Found nack
            return Err(Box::new(ImonError::CommandNack));
        }
        if buffer_single[0] == 0x06 {
            // Found ack
            return Ok(());
        }
    }

    Err(Box::new(ImonError::UnexpectedResponse)) // Found neither
}

fn auto_save_spectrum(spectrum: &Spectrum, folder_path: &PathBuf) -> Result<u32, Box<dyn Error>> {
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

// Region: Spectrum creation ---------------------------------------------------

impl Spectrum {
    pub fn from_ibsen_imon(
        pixel_readings: &[u32],
        temperature: f64,
        coefficients: &ImonCoefficients,
        dark_pixels: &[u32],
    ) -> Spectrum {
        let t_alpha = coefficients.temperature[0];
        let t_alpha_0 = coefficients.temperature[1];
        let t_beta = coefficients.temperature[2];
        let t_beta_0 = coefficients.temperature[3];

        let mut values: Vec<SpectrumValue> = Vec::new();

        for (pixel, reading) in pixel_readings.iter().enumerate() {
            let linear = false;
            let pwr: f64 = match linear {
                true => *reading as f64,
                false => {
                    let reading_compensated = (*reading as f64) - (dark_pixels[pixel] as f64);
                    let mut reading_compensated = reading_compensated;
                    if reading_compensated < 1.0 {
                        reading_compensated = 1.0;
                    }

                    10.0 * reading_compensated.log10() // for power: db = 10 log10(linear)
                }
            };

            let mut wl: f64 = 0.0;

            for (j, coef) in coefficients.wavelength.iter().enumerate() {
                wl += (pixel as f64).powf(j as f64) * coef;
            }

            wl = (wl - t_beta * temperature - t_beta_0) / (1.0 + t_alpha * temperature + t_alpha_0);

            values.push(SpectrumValue {
                wavelength: wl * 1e-9,
                power: pwr,
            });
        }

        let values = values.into_iter().rev().collect();
        Spectrum::from_values(values)
    }
}
