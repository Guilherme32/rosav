// Region Config -----------------------------------------------------------------------------------

use std::fs::{ read_to_string, write, create_dir_all };
use std::path::{ Path, PathBuf };
use std::error::Error;

use toml;
use home::home_dir;

use crate::spectrum_handler::{
    HandlerConfig,
    acquisitors::file_reader::FileReaderConfig,
    AcquisitorSimple,
    AcquisitorConfig
};


// Region: Main config ------------------------------------------------------

pub fn handler_config_path() -> PathBuf {
    let home = match home_dir() {
        Some(path) => path,
        None => Path::new("./").to_path_buf()            // If can't find home, uses config on pwd
    };

    let path = home.join(".config/rosa/handler.toml");
    path
}

pub fn load_handler_config() -> Result<HandlerConfig, Box<dyn Error>> {
    let text = read_to_string(&handler_config_path())?;
    let config: HandlerConfig = toml::from_str(&text)?;

    Ok(config)
}

pub fn write_handler_config(config: &HandlerConfig) -> Result<(), Box<dyn Error>> {
    let config_path = handler_config_path();

    if let Some(parent) = config_path.parent() {            // Enforces the parent folder
        create_dir_all(parent)?;
    }
    write(&config_path, &toml::to_string(config)?)?;

    Ok(())
}

pub fn load_acquisitor_config(acquisitor_type: AcquisitorSimple) -> Result<AcquisitorConfig, Box<dyn Error>> {
    match acquisitor_type {
        AcquisitorSimple::FileReader =>
            Ok(AcquisitorConfig::FileReaderConfig(load_file_reader_config()?))
    }
}

pub fn write_acquisitor_config(config: &AcquisitorConfig) -> Result<(), Box<dyn Error>> {
    match config {
        AcquisitorConfig::FileReaderConfig(config) =>
            write_file_reader_config(&config),
        AcquisitorConfig::Other(_) => Ok(())
    }
}


// Region: Acquisitors config --------------------------------------------------
// Subregion: file_reader config -----------------------------------------------

pub fn file_reader_config_path() -> PathBuf {
    let home = match home_dir() {
        Some(path) => path,
        None => Path::new("./").to_path_buf()            // If can't find home, uses config on pwd
    };

    let path = home.join(".config/rosa/file_reader.toml");
    path
}

pub fn load_file_reader_config() -> Result<FileReaderConfig, Box<dyn Error>> {
    let text = read_to_string(&file_reader_config_path())?;
    let config: FileReaderConfig = toml::from_str(&text)?;

    Ok(config)
}

pub fn write_file_reader_config(config: &FileReaderConfig) -> Result<(), Box<dyn Error>> {
    let config_path = file_reader_config_path();

    if let Some(parent) = config_path.parent() {            // Enforces the parent folder
        create_dir_all(parent)?;
    }
    write(&config_path, &toml::to_string(config)?)?;

    Ok(())
}

