// Region Config -----------------------------------------------------------------------------------

use std::error::Error;
use std::fs::{create_dir_all, read_to_string, write};
use std::path::{Path, PathBuf};

use home::home_dir;
use toml;

use crate::spectrum_handler::{
    acquisitors::{file_reader::FileReaderConfig, ibsen_imon::ImonConfig},
    AcquisitorConfig, AcquisitorSimple, HandlerConfig,
};

// Region: Main config ------------------------------------------------------

pub fn handler_config_path() -> PathBuf {
    let home = match home_dir() {
        Some(path) => path,
        None => Path::new("./").to_path_buf(), // If can't find home, uses config on pwd
    };

    home.join(".config/rosa/handler.toml")
}

pub fn load_handler_config() -> Result<HandlerConfig, Box<dyn Error>> {
    let text = read_to_string(handler_config_path())?;
    let mut config: HandlerConfig = toml::from_str(&text)?;

    config.wavelength_limits = None; // It doesn't really make a lot of sense to keep
    config.power_limits = None; // the limits between uses

    Ok(config)
}

pub fn write_handler_config(config: &HandlerConfig) -> Result<(), Box<dyn Error>> {
    let config_path = handler_config_path();

    if let Some(parent) = config_path.parent() {
        // Enforces the parent folder
        create_dir_all(parent)?;
    }
    write(&config_path, toml::to_string(config)?)?;

    Ok(())
}

pub fn load_acquisitor_config(
    acquisitor_type: AcquisitorSimple,
) -> Result<AcquisitorConfig, Box<dyn Error>> {
    match acquisitor_type {
        AcquisitorSimple::FileReader => Ok(AcquisitorConfig::FileReaderConfig(
            load_file_reader_config()?,
        )),
        AcquisitorSimple::Imon => Ok(AcquisitorConfig::ImonConfig(load_imon_config()?)),
    }
}

pub fn write_acquisitor_config(config: &AcquisitorConfig) -> Result<(), Box<dyn Error>> {
    match config {
        AcquisitorConfig::FileReaderConfig(config) => write_file_reader_config(config),
        AcquisitorConfig::ImonConfig(config) => write_imon_config(config),
    }
}

// Region: Acquisitors config --------------------------------------------------
// Subregion: file_reader config -----------------------------------------------

pub fn file_reader_config_path() -> PathBuf {
    let home = match home_dir() {
        Some(path) => path,
        None => Path::new("./").to_path_buf(), // If can't find home, uses config on pwd
    };

    home.join(".config/rosa/file_reader.toml")
}

pub fn load_file_reader_config() -> Result<FileReaderConfig, Box<dyn Error>> {
    let text = read_to_string(file_reader_config_path())?;
    let config: FileReaderConfig = toml::from_str(&text)?;

    Ok(config)
}

pub fn write_file_reader_config(config: &FileReaderConfig) -> Result<(), Box<dyn Error>> {
    let config_path = file_reader_config_path();

    if let Some(parent) = config_path.parent() {
        // Enforces the parent folder
        create_dir_all(parent)?;
    }
    write(&config_path, toml::to_string(config)?)?;

    Ok(())
}

// Subregion: imon config -----------------------------------------------

pub fn imon_config_path() -> PathBuf {
    let home = match home_dir() {
        Some(path) => path,
        None => Path::new("./").to_path_buf(), // If can't find home, uses config on pwd
    };

    home.join(".config/rosa/imon.toml")
}

pub fn load_imon_config() -> Result<ImonConfig, Box<dyn Error>> {
    let text = read_to_string(imon_config_path())?;
    let config: ImonConfig = toml::from_str(&text)?;

    Ok(config)
}

pub fn write_imon_config(config: &ImonConfig) -> Result<(), Box<dyn Error>> {
    let config_path = imon_config_path();

    if let Some(parent) = config_path.parent() {
        // Enforces the parent folder
        create_dir_all(parent)?;
    }
    write(&config_path, toml::to_string(config)?)?;

    Ok(())
}
