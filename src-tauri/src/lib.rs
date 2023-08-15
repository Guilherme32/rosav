use chrono::prelude::*;
use home::home_dir;
use log::{error, info, warn};
use serde::{Deserialize, Serialize};
use std::fs::create_dir_all;
use std::path::{Path, PathBuf};
use std::sync::mpsc::SyncSender;

pub mod api;
pub mod config;
pub mod spectrum;
pub mod spectrum_handler;

#[derive(Serialize, Deserialize, Debug)]
pub struct Log {
    msg: String,
    log_type: LogType,
}

#[derive(Serialize, Deserialize, Debug)]
pub enum LogType {
    Info,
    Warning,
    Error,
}

fn get_log_path() -> PathBuf {
    if let Some(home) = home_dir() {
        let path = home.join(".config").join("rosav");
        if create_dir_all(path.clone()).is_ok() {
            return path.join("rosav.log");
        }
    }

    Path::new("rosav.log").to_path_buf() // If anything fails, create at the app's location
}

pub fn setup_fern_logger() -> Result<(), fern::InitError> {
    let log_path = get_log_path();

    fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[#{} at {} on {}] {}",
                record.level(),
                Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.target(),
                message
            ))
        })
        .level(log::LevelFilter::Info)
        .chain(fern::log_file(log_path)?)
        .apply()?;

    Ok(())
}

pub fn new_log(msg: String, log_type: LogType) -> Log {
    Log { msg, log_type }
}

pub fn log_info(tx: &SyncSender<Log>, msg: String) {
    info!("{}", msg);
    println!("#Info: {}", msg);
    match tx.send(new_log(msg, LogType::Info)) {
        Ok(_) => (),
        Err(error) => println!(
            "#Exteme error: Error when trying to send info log! ({})",
            error
        ),
    }
}

pub fn log_war(tx: &SyncSender<Log>, msg: String) {
    warn!("{}", msg);
    println!("#Warning: {}", msg);
    match tx.send(new_log(msg, LogType::Warning)) {
        Ok(_) => (),
        Err(error) => println!(
            "#Exteme error: Error when trying to send warning log! ({})",
            error
        ),
    }
}

pub fn log_error(tx: &SyncSender<Log>, msg: String) {
    error!("{}", msg);
    println!("#Error: {}", msg);
    match tx.send(new_log(msg, LogType::Error)) {
        Ok(_) => (),
        Err(error) => println!(
            "#Exteme error: Error when trying to send error log! ({})",
            error
        ),
    }
}
