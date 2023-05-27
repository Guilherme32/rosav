use serde::{Serialize, Deserialize};
use std::path::PathBuf;


#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum AcquisitorSimple {
    FileReader
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum AcquisitorConfig {
    FileReaderConfig(FileReaderConfig),
    Other(u32)                            // TODO just here to supress a warning, remove when add the other
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct FileReaderConfig {
    pub watcher_path: PathBuf
}

pub fn empty_file_reader_config() -> FileReaderConfig {
    FileReaderConfig {
        watcher_path: PathBuf::new()
    }
}
