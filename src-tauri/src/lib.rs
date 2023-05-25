use serde::{ Serialize, Deserialize };
use std::sync::mpsc::SyncSender;


pub mod file_reader;
pub mod spectrum;
pub mod api;

#[derive(Serialize, Deserialize, Debug)]
pub struct Log {
    // id: u32,
    msg: String,
    log_type: LogType
}

#[derive(Serialize, Deserialize, Debug)]
pub enum LogType {
    Info,
    Warning,
    Error
}

pub fn new_log(msg: String, log_type: LogType) -> Log {
    Log {
        // id: 0,
        msg,
        log_type
    }
}

pub fn log_info(tx: &SyncSender<Log>, msg: String) {
    println!("#Info: {}", msg);
    match tx.send(new_log(msg, LogType::Info)) {
        Ok(_) => (),
        Err(error) => println!("#Exteme error: Error when trying to send info log! ({})", error)
    }
}

pub fn log_war(tx: &SyncSender<Log>, msg: String) {
    println!("#Warning: {}", msg);
    match tx.send(new_log(msg, LogType::Warning)) {
        Ok(_) => (),
        Err(error) => println!("#Exteme error: Error when trying to send warning log! ({})", error)
    }
}

pub fn log_error(tx: &SyncSender<Log>, msg: String) {
    println!("#Error: {}", msg);
    match tx.send(new_log(msg, LogType::Error)) {
        Ok(_) => (),
        Err(error) => println!("#Exteme error: Error when trying to send error log! ({})", error)
    }
}


// Region Config -----------------------------------------------------------------------------------

use std::fs::{ read_to_string, write };
use std::path::{ Path, PathBuf };
use std::error::Error;

use toml;
use home::home_dir;

use file_reader::FileReaderConfig;

pub fn config_path() -> PathBuf {
    let home = match home_dir() {
        Some(path) => path,
        None => Path::new("./").to_path_buf()            // If can't find home, uses config on pwd
    };

    let path = home.join(".config/rosa.toml");
    path
}

pub fn get_config() -> Result<FileReaderConfig, Box<dyn Error>> {
    let text = read_to_string(&config_path())?;
    let config: FileReaderConfig = toml::from_str(&text)?;

    Ok(config)
}

pub fn write_config(config: &FileReaderConfig) -> Result<(), Box<dyn Error>> {
    write(&config_path(), &toml::to_string(config)?)?;

    Ok(())
}

pub fn default_config() -> FileReaderConfig {
    FileReaderConfig {
        watcher_path: "D:/test/read".to_string().into(),
        auto_save_path: "D:/test/save".to_string().into(),
        wavelength_limits: None,
        power_limits: None
    }
}

