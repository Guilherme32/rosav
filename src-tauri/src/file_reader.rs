#![allow(dead_code)]

use std::path::Path;
use notify;
use notify::{ Watcher, RecursiveMode };
use std::fs::File;
use std::io::Read;
use std::io;
use std::error::Error;

use std::time::Duration;
use std::thread::sleep;
use std::sync::{Arc, Mutex};
use std::sync::atomic::{ self, AtomicBool };

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
enum ReaderState {
    Disconnected,
    Connected,
    Reading (notify::RecommendedWatcher)
}

#[derive(Debug)]
pub struct FileReader {
    pub path: String,
    pub last_spectrum: Arc<Mutex<Option<Spectrum>>>,
    pub frozen_spectra: Vec<Spectrum>,
    pub unread_spectrum: Arc<AtomicBool>,
    pub spectrum_limits: Mutex<Option<Limits>>,
    state: ReaderState
}

#[derive(Debug)]
pub enum ConnectError {
    ReaderAlreadyConnected,
    PathDoesNotExist,
    PathIsNotDir,
    PathWithoutPermission
}

#[derive(Debug)]
pub enum ContinuousError {
    ReaderNotConnected,
    ReaderAlreadyContinuous,
    NotifyInternalError
}

impl FileReader {
    pub fn connect<'a>(&'a mut self) -> Result<&'a mut Self, ConnectError>
    {
        match self.state {
            ReaderState::Disconnected => (),
            _ => return Err(ConnectError::ReaderAlreadyConnected)
        }

        let path = Path::new(&self.path);

        match path.try_exists() {
            Err(_) => { return Err(ConnectError::PathWithoutPermission) },
            Ok(exists) => {
                if !exists {
                    return Err(ConnectError::PathDoesNotExist)
                }
            }
        }

        if !path.is_dir() {
            return Err(ConnectError::PathIsNotDir);
        }

        self.state = ReaderState::Connected;
        Ok(self)
    }

    pub fn read_continuous<'a>(&'a mut self) -> Result<&'a mut Self, ContinuousError>
    {
        match self.state {
            ReaderState::Disconnected => return Err(ContinuousError::ReaderNotConnected),
            ReaderState::Reading(_) => return Err(ContinuousError::ReaderAlreadyContinuous),
            _ => ()
        }

        let path = Path::new(&self.path);

        let spectrum_reference = Arc::clone(&self.last_spectrum);
        let flag_reference = Arc::clone(&self.unread_spectrum);
        let callback = move |event| watcher_callback(
            event,
            Arc::clone(&spectrum_reference),
            Arc::clone(&flag_reference)
        );

        let mut watcher = match notify::recommended_watcher(callback) {
            Ok(_watcher) => _watcher,
            Err(_) => return Err(ContinuousError::NotifyInternalError)
        };

        match watcher.watch(path, RecursiveMode::NonRecursive) {
            Ok(_) => (),
            Err(_) => return Err(ContinuousError::NotifyInternalError)
        }

        self.state = ReaderState::Reading(watcher);
        Ok(self)
    }


    pub fn get_last_spectrum_path(&self, svg_limits: (u32, u32)) -> Option<String> {
        let spectrum = match self.last_spectrum.lock() {
            Ok(spectrum) => spectrum,
            Err(error) => { 
                println!("[FR] Could not acquire the lock to get last spectrum ({})", error);
                return None;
            }
        };

        let spec_limits = match self.spectrum_limits.lock() {
            Ok(spec_limits) => spec_limits,
            Err(error) => { 
                println!("[FUL] Could not acquire the lock to get last limits ({})", error);
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
            Err(error) => { 
                println!("[FUL] Could not acquire the lock to get last spectrum ({})", error);
                return ();
            }
        };

        let mut limits = match self.spectrum_limits.lock() {
            Ok(limits) => limits,
            Err(error) => { 
                println!("[FUL] Could not acquire the lock to get last limits ({})", error);
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

}

pub fn new_file_reader(path: String) -> FileReader {
    FileReader {
        path,
        last_spectrum: Arc::new(Mutex::new(None)),
        frozen_spectra: vec![],
        unread_spectrum: Arc::new(AtomicBool::new(false)),
        spectrum_limits: Mutex::new(None),
        state: ReaderState::Disconnected
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
    new_spectrum: Arc<AtomicBool>
) {
    let event = match response {
        Ok(event) if event.kind.is_create() => event,
        Ok(_) => return,                    // Don't care about successfull non create events
        Err(error) => return println!("[FR] watch error: {:?}", error)
    };

    let text = match read_file_event(&event) {
        Ok(text) => text,
        Err(error) => return println!(
            "[FR] Could not react to the create file event: {:?},\
            \nError: {}",
            event, error)
    };

    let spectrum = match Spectrum::from_str(&text) {
        Ok(spectrum) => spectrum,
        Err(error) => return println!("[FR] Could not transform the file\
            into a spectrum ({})", error)
    };

    match last_spectrum.lock() {
        Ok(mut last_spectrum) => {
            *last_spectrum = Some(spectrum);
            new_spectrum.store(true, atomic::Ordering::Relaxed);
        },
        Err(error) => println!("[FR] Could not acquire the spectrum lock ({})", error)
    };
}

