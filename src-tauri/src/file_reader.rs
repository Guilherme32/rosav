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

use crate::spectrum::Spectrum;

pub fn test() {
    println!("funcao de teste");
    return ();
}

#[derive(Debug)]
pub struct Connected;
#[derive(Debug)]
pub struct Disconnected;
#[derive(Debug)]
pub struct Continuous {
    watcher: notify::RecommendedWatcher
}

#[derive(Debug)]
pub struct FileReader<T> {
    pub path: String,
    pub last_spectrum: Arc<Mutex<Option<Spectrum>>>,
    pub frozen_spectra: Vec<Spectrum>,
    pub unread_spectrum: Arc<AtomicBool>,
    connection: T
}

#[derive(Debug)]
pub enum ConnectError {
    PathDoesNotExist,
    PathIsNotDir,
    PathWithoutPermission
}

#[derive(Debug)]
pub enum NotifyError {
    NotifyInternalError
}

impl FileReader<Disconnected> {
    pub fn connect(self) 
        -> Result<FileReader<Connected>, (ConnectError, FileReader<Disconnected>)>
    {
        let path = Path::new(&self.path);

        match path.try_exists() {
            Err(_) => { return Err((ConnectError::PathWithoutPermission, self)) },
            Ok(exists) => {
                if !exists {
                    return Err((ConnectError::PathDoesNotExist, self))
                }
            }
        }

        if !path.is_dir() {
            return Err((ConnectError::PathIsNotDir, self));
        }

        Ok(FileReader {
            path: self.path,
            last_spectrum: self.last_spectrum,
            frozen_spectra: self.frozen_spectra,
            unread_spectrum: self.unread_spectrum,
            connection: Connected
        })
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

impl FileReader<Connected> {
    pub fn read_continuous(self)
        -> Result<FileReader<Continuous>, (NotifyError, FileReader<Connected>)>
    {
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
            Err(_) => return Err((NotifyError::NotifyInternalError, self))
        };

        match watcher.watch(path, RecursiveMode::NonRecursive) {
            Ok(_) => (),
            Err(_) => return Err((NotifyError::NotifyInternalError, self))
        }

        Ok(FileReader {
            path: self.path,
            last_spectrum: self.last_spectrum,
            frozen_spectra: self.frozen_spectra,
            unread_spectrum: self.unread_spectrum,
            connection: Continuous { watcher }
        })
    }
}

pub fn new_file_reader(path: String) -> FileReader<Disconnected> {
    FileReader {
        path,
        last_spectrum: Arc::new(Mutex::new(None)),
        frozen_spectra: vec![],
        unread_spectrum: Arc::new(AtomicBool::new(false)),
        connection: Disconnected
    }
}

impl FileReader<Continuous> {
    pub fn get_last_spectrum_path(&self, svg_limits: (f64, f64)) -> Option<String> {
        let spectrum = match self.last_spectrum.lock() {
            Ok(spectrum) => spectrum,
            Err(error) => { 
                println!("[FR] Could not acquire the lock to get last spectrum ({})", error);
                return None;
            }
        };

        self.unread_spectrum.store(false, atomic::Ordering::Relaxed);
        match &*spectrum {
            Some(spectrum) => Some(spectrum.to_path(svg_limits)),
            None => None
        }
    }
}
