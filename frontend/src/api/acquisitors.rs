use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum AcquisitorSimple {
    FileReader,
    Imon,
    Example
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum AcquisitorConfig {
    FileReaderConfig(FileReaderConfig),
    ImonConfig(ImonConfig),
    ExampleConfig(ExampleConfig),
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
    pub multisampling: u32,
}

pub fn empty_imon_config() -> ImonConfig {
    ImonConfig {
        exposure_ms: 0.0,
        read_delay_ms: 0,
        multisampling: 0,
    }
}

// -----------------------------------------------------------------------------

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ExampleConfig {
    pub points: u64,
    pub amplitude: f64,
    pub phase_t_speed: f64,
    pub phase_x_speed: f64,
    pub update_delay_millis: u64,
}

pub fn empty_example_config() -> ExampleConfig {
    ExampleConfig {
        points: 0,
        amplitude: 0.0,
        phase_t_speed: 0.0,
        phase_x_speed:0.0,
        update_delay_millis: 0,
    }
}
