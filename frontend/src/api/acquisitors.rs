use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum AcquisitorSimple {
    FileReader,
    Imon,
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum AcquisitorConfig {
    FileReaderConfig(FileReaderConfig),
    ImonConfig(ImonConfig),
}

// Region: Configs -------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct FileReaderConfig {
    pub watcher_path: PathBuf,
}

pub fn empty_file_reader_config() -> FileReaderConfig {
    FileReaderConfig {
        watcher_path: PathBuf::new(),
    }
}

// -----------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ImonConfig {
    pub exposure_ms: f64,
    pub read_delay_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ImonCalibration {
    pub wavelength_fit: [f64; 6],
    pub temperature_coeffs: [f64; 4],
}

pub fn empty_imon_config() -> ImonConfig {
    ImonConfig {
        exposure_ms: 0.0,
        read_delay_ms: 0,
    }
}
