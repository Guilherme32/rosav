use std::path::PathBuf;
use std::{fs, io};
use std::error::Error;
use std::sync::mpsc::SyncSender;
use crate::config::load_example_config;
use crate::{
    Log,
    log_war,
    config::{
        load_file_reader_config,
        load_imon_config
    }
};

pub mod file_reader;
pub mod ibsen_imon;
pub mod example;

use crate::spectrum_handler::{ AcquisitorSimple, Acquisitor , SpectrumHandler, State, Spectrum };

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
        },
        AcquisitorSimple::Imon => {
            let config = match load_imon_config() {
                Ok(config) => config,
                Err(error) => {
                    log_war(&log_tx, format!("[QLA] Não foi possível ler a \
                        config. do Imon. Usando a padrão. Erro: \
                        {}", error));
                    ibsen_imon::default_config()
                }
            };

            Acquisitor::Imon(ibsen_imon::new_imon(config, log_tx))
        }
        AcquisitorSimple::Example => {
            let config = match load_example_config() {
                Ok(config) => config,
                Err(error) => {
                    log_war(&log_tx, format!("[QLA] Não foi possível ler a \
                        config. do Example. Usando a padrão. Erro: \
                        {}", error));
                    example::default_config()
                }
            };

            Acquisitor::Example(example::new_example(config, log_tx))
        }
    }
}


// Shared behaviour -------------------------------------------------------------

/// Trait que deve ser implementado por qualquer aquisitor. As funções de transição devem alterar a
/// variável de estado. A variável de estado deve ser implementada para o próprio aquisitor,
/// mantendo em mente que ela dever possível de ser traduzida para o 'State' na função
/// 'get_simplified_state'. Isso é feito para permitir que objetos sejam guardados no enum de
/// estado, garantindo que sejam dealocados quando houver uma transição. Todas as possíveis
/// transições devem ser tratadas e executadas no código específico do aquisitor, seja pelas
/// chamadas síncronas ou pela thread de leitura.
pub trait AcquisitorTrait {
    /// Tenta realizar a conexão do aquisitor
    fn connect(&self) -> Result<(), impl Error + 'static>;

    /// Tenta realizar a desconexão do aquisitor. Deve matar quaisquer threads criadas pelo
    /// aquisitor e liberar as referências, caso existam.
    fn disconnect(&self) -> Result<(), impl Error + 'static>;

    /// Inicia a leitura contínua do aquisitor. Cabe ao próprio aquisitor decidir a sua taxa de
    /// aquisição. Uma nova thread deve ser iniciada para realizar as leituras sem bloquear o
    /// programa. As variáveis que devem ser passadas para essa thread estão todas em formas
    /// compartilháveis na estrutura do handler e devem ter as referências copiadas. Sempre que uma
    /// nova leitura for realizada, o seguinte deve ser feito em ordem:
    /// - Caso a variável booleana 'saving' new seja True: O espectro novo deve ser salvo em um
    ///     arquivo através da função auto_save_spectrum (desse arquivo)
    /// - O novo espectro lido deve ser armazenado na variável protegida por mutex 'last_spectrum'
    /// - A variável booleana 'unread_spectrum' deve ser atualizada para True
    ///
    /// O handler faz o polling do 'unread_spectrum' seguindo os pedidos do frontend. Se for True,
    /// ele envia envia o 'last spectrum' para o frontend, caso contrário não envia nada.
    fn start_reading(&self, handler: &SpectrumHandler, single_read: bool,) -> Result<(), impl Error + 'static>;

    /// Interrompe a leitura contínua do aquisitor. Deve encerrar a thread criada no start_reading,
    /// liberando as referências criadas para a memória compartilhada.
    fn stop_reading(&self) -> Result<(), impl Error + 'static>;

    /// Retorna o estado simplificado, o enum que representa o estado sem carregar as variáveis
    /// adicionais necessárias para o funcionamento
    fn get_simplified_state(&self) -> State;
}

pub fn auto_save_spectrum(spectrum: &Spectrum, folder_path: &PathBuf) -> Result<u32, Box<dyn Error>> {
    fs::create_dir_all(folder_path)?;

    for i in 0..100_000 {
        let new_path = folder_path.join(format!("spectrum{:03}.txt", i));
        if !new_path.exists() {
            spectrum.save(&new_path)?;
            return Ok(i);
        }
    }

    Err(Box::new(io::Error::new(
        io::ErrorKind::Other,
        "Overflow de espectros,\
        o programa só suporta até 'spectrum99999'",
    )))
}
