use std::sync::mpsc::SyncSender;
use crate::{ Log, log_war, config::load_file_reader_config };

pub mod file_reader;

use crate::spectrum_handler::{ AcquisitorSimple, Acquisitor };

pub fn load_acquisitor(acquisitor_type: &AcquisitorSimple, log_tx: SyncSender<Log>) -> Acquisitor {
    match acquisitor_type {
        AcquisitorSimple::FileReader => {
            let config = match load_file_reader_config() {
                Ok(config) => config,
                Err(error) => {
                    log_war(&log_tx, format!("[QLA] Não foi possível ler a \
                        config. do Leitor de Arquivos. Usando a padrão. Erro: \
                        {}", error));
                    file_reader::default_config()
                } 
            };
            
            Acquisitor::FileReader(file_reader::new_file_reader(config, log_tx))
        }
    }
}

