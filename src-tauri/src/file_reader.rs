#![allow(dead_code)]

use std::path::Path;
use notify;
use notify::{ Watcher, RecursiveMode };
use std::fs::File;
use std::io::Read;
use std::error::Error;

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
    pub last_spectrum: Option<Spectrum>,
    pub frozen_spectra: Vec<Spectrum>,
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
            connection: Connected
        })
    }
}

fn read_file_event(event: &notify::Event) -> Result<(), Box<dyn Error>> {
    println!("event: {:?}", event);
    if event.kind.is_create() {
        let path = &event.paths[0];
        let mut file = File::open(path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        println!("In file: {}", &contents);
    }

    Ok(())
}

impl FileReader<Connected> {
    pub fn read_continuous(self)
        -> Result<FileReader<Continuous>, (NotifyError, FileReader<Connected>)>
    {
        let path = Path::new(&self.path);

        let watcher_callback = |response: Result<notify::Event, _>| {
            match response {
                Ok(event) => {
                    if let Err(error) = read_file_event(&event) {
                        println!("Could not react to the file event {:?}, \n Error: {}",
                            event, error);
                    }
                },
                Err(error) => println!("Watch error: {:?}", error)
            };
            ()
        };

        let mut watcher = match notify::recommended_watcher(watcher_callback) {
            Ok(_watcher) => _watcher,
            Err(_) => { return Err((NotifyError::NotifyInternalError, self)); }
        };

        match watcher.watch(path, RecursiveMode::NonRecursive) {
            Ok(_) => (),
            Err(_) => { return Err((NotifyError::NotifyInternalError, self)); }
        }

        Ok(FileReader {
            path: self.path,
            last_spectrum: self.last_spectrum,
            frozen_spectra: self.frozen_spectra,
            connection: Continuous { watcher }
        })
    }
}

pub fn new_file_reader(path: String) -> FileReader<Disconnected> {
    FileReader {
        path,
        last_spectrum: None,
        frozen_spectra: vec![],
        connection: Disconnected
    }
}
