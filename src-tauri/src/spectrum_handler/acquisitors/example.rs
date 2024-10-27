#![allow(refining_impl_trait)]

use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{mpsc::SyncSender, Arc, Mutex};
use std::time::{self, Duration};

use serde::{Deserialize, Serialize};

use crate::spectrum::{Spectrum, SpectrumValue};
use crate::spectrum_handler::State;
use crate::{log_error, log_info, log_war, Log};
use crate::spectrum_handler::acquisitors::AcquisitorTrait;
use std::thread;

use super::auto_save_spectrum;

// Region: Main declarations --------------------------------------------------

#[derive(Debug, thiserror::Error)]
pub enum ExampleErrors {
    #[error("Não conectou")]
    ConnectFail,
}

#[derive(Debug)]
pub enum ExampleState {
    Disconnected,
    Connected(u32),
    Reading(mpsc::Sender<ExampleConfig>),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExampleConfig {
    points: u64,
    amplitude: f64,
    phase_t_speed: f64,
    phase_x_speed: f64,
    update_delay_millis: u64,
}

#[derive(Debug)]
pub struct Example {
    state: Arc<Mutex<ExampleState>>,
    pub log_sender: SyncSender<Log>,
    pub config: Mutex<ExampleConfig>,
}

// Region: Default generators -------------------------------------------------

pub fn new_example(config: ExampleConfig, log_sender: SyncSender<Log>) -> Example {
    Example {
        state: Arc::new(Mutex::new(ExampleState::Disconnected)),
        log_sender,
        config: Mutex::new(config),
    }
}

pub fn default_config() -> ExampleConfig {
    ExampleConfig {
        points: 1024,
        amplitude: 2.0,
        phase_t_speed: 6.14,
        phase_x_speed: 6.14,
        update_delay_millis: 100,
    }
}

// Region: Necessary impls ----------------------------------------------------

impl AcquisitorTrait for Example {
    fn connect(&self) -> Result<(), ExampleErrors> {
        let mut state = self.state.lock().unwrap();

        if let ExampleState::Disconnected = *state {
            *state = ExampleState::Connected(3);
        } else {
            log_error(&self.log_sender, "[AEC] Não foi possível conectar. Aquisitor não está desconectado".into());
            return Err(ExampleErrors::ConnectFail);
        }

        log_info(&self.log_sender, "[AEC] Aquisitor de exemplo conectado".into());
        Ok(())
    }

    fn disconnect(&self) -> Result<(), ExampleErrors> {
        let mut state = self.state.lock().unwrap();

        if let ExampleState::Disconnected = *state {
            log_error(&self.log_sender, "[AEC] Não foi possível desconectar. Aquisitor já está desconectado".into());
            return Err(ExampleErrors::ConnectFail);
        } else {
            *state = ExampleState::Disconnected;
        }

        log_info(&self.log_sender, "[AEC] Aquisitor de exemplo desconectado".into());
        Ok(())
    }

    fn start_reading(&self, handler: &crate::spectrum_handler::SpectrumHandler, single_read: bool,) -> Result<(), ExampleErrors> {
        let acq_config = self.config.lock().unwrap();

        if single_read {
            let mut last_spectrum = handler.last_spectrum.lock().unwrap();
            *last_spectrum = Some(get_example_spectrum(&acq_config));
            handler.unread_spectrum.store(true, std::sync::atomic::Ordering::Relaxed);
            return Ok(());
        }

        let handler_config = handler.config.lock().unwrap();
        let mut state = self.state.lock().unwrap();

        let (config_tx, config_rx) = mpsc::channel();

        let args = ConstantReadArgs {
            last_spectrum: handler.last_spectrum.clone(),
            unread_spectrum_flag: handler.unread_spectrum.clone(),
            saving: handler.saving_new.clone(),
            auto_save_path: handler_config.auto_save_path.clone(),
            log_tx: self.log_sender.clone(),
            state: self.state.clone(),
            config_rx
        };

        let _ = config_tx.send((*acq_config).clone());
        *state = ExampleState::Reading(config_tx);

        thread::spawn(move || constant_read(args));

        log_info(&self.log_sender, "[AEC] Aquisitor de exemplo começou a ler".into());
        Ok(())
    }

    fn stop_reading(&self) -> Result<(), ExampleErrors> {
        let mut state = self.state.lock().unwrap();
        *state = ExampleState::Connected(9);

        log_info(&self.log_sender, "[AEC] Aquisitor de exemplo parou de ler".into());

        Ok(())
    }

    fn get_simplified_state(&self) -> crate::spectrum_handler::State {
        let state = self.state.lock().unwrap();

        match &*state {
            ExampleState::Disconnected => State::Disconnected,
            ExampleState::Connected(_) => State::Connected,
            ExampleState::Reading(_) => State::Reading,
        }
    }
}

impl Example {
    pub fn update_config(&self, new_config: ExampleConfig) {
        let mut config = self.config.lock().unwrap();
        let state = self.state.lock().unwrap();

        if let ExampleState::Reading(config_tx) = &*state {
            let _ = config_tx.send(new_config.clone());
        }

        *config = new_config;
    }

    pub fn get_config(&self) -> ExampleConfig {
        let config = self.config.lock().unwrap();

        (*config).clone()
    }
}

// Region: Outside impls ------------------------------------------------------

struct ConstantReadArgs {
    last_spectrum: Arc<Mutex<Option<Spectrum>>>,
    unread_spectrum_flag: Arc<AtomicBool>,
    saving: Arc<AtomicBool>,
    auto_save_path: PathBuf,
    log_tx: SyncSender<Log>,
    state: Arc<Mutex<ExampleState>>,
    config_rx: Receiver<ExampleConfig>,
}

fn constant_read(args: ConstantReadArgs) {
    let mut config = ExampleConfig {
        points: 0,
        amplitude: 0.0,
        phase_t_speed: 0.0,
        phase_x_speed: 0.0,
        update_delay_millis: 0,
    };

    loop {
        {   // This is to make sure the locks drop before the sleep
            match args.config_rx.try_recv() {
                Ok(new_config) => {
                    config = new_config;
                }
                Err(TryRecvError::Empty) => (),
                // Esse braço garante que a thread vai morrer assim que o estado for alterado, visto
                // que o transmissor de configuração vai ser desalocado
                Err(TryRecvError::Disconnected) => return,
            }

            let spectrum = get_example_spectrum(&config);

            if args.saving.load(std::sync::atomic::Ordering::Relaxed)
                && auto_save_spectrum(&spectrum, &args.auto_save_path).is_err()
            {
                log_war(&args.log_tx, "[AER] Falha ao salvar espectro automaticamente, interrompendo
                    leitura".into());

                let mut state = args.state.lock().unwrap();
                *state = ExampleState::Disconnected;
                return;
            }

            let mut last_spectrum = args.last_spectrum.lock().unwrap();
            *last_spectrum = Some(spectrum);
            args.unread_spectrum_flag.store(true, std::sync::atomic::Ordering::Relaxed);
        }

        thread::sleep(Duration::from_millis(config.update_delay_millis));
    }

}

fn get_example_spectrum(config: &ExampleConfig) -> Spectrum {
    let time: u128 = time::SystemTime::now().duration_since(time::UNIX_EPOCH).unwrap().as_millis();
    let time = (time % 3_600_000) as f64;
    let time = time / 3600.0;

    let spec_vec = (0..config.points)
        .map(|x| x as f64)
        .map(|x| x / (config.points as f64))
        .map(|x| (x, time * config.phase_t_speed + x * config.phase_x_speed))
        .map(|(x, y)| (x, config.amplitude * y.cos()))
        .map(|(x, y)| SpectrumValue{wavelength: x*std::f64::consts::PI, power: y})
        .collect::<Vec<SpectrumValue>>();

    Spectrum::from_values(spec_vec)
}
