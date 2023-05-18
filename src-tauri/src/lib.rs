use serde::{ Serialize, Deserialize };


pub mod file_reader;

pub mod spectrum;

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

