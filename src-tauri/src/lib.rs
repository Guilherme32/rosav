use serde::{ Serialize, Deserialize };
use std::sync::mpsc::SyncSender;


pub mod spectrum_handler;
pub mod spectrum;
pub mod api;
pub mod config;
pub mod processing;

#[derive(Serialize, Deserialize, Debug)]
pub struct Log {
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


