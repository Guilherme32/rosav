---
geometry: margin=15mm
...

//! Autor: Guilherme Sampaio

# Tutorial para adicionar um novo aquisitor
Os novos aquisitores devem ser adicionados no código diretamente, não há uma
interface de extensão que permite a integração a um programa já compilado. O
aquisitor deve ser definido com o seu comportamento adequado no backend e,
posteriormente, adicionado às funções adequadas pelo backend. Do lado do
frontend, a única parte que realmente importa é a definição para a parte de
configurações. O passo a passo para adicionar está descrito abaixo, e
o aquisitor definido aqui pode ser encontrado no sistema. Ele não é visível
para o usuário em uma compilação direta, mas pode ser testado compilando o
frontend com o feature 'example'.

``` trunk build --features example ```

Isso ocorre porque a opção que permite a seleção dele pela configuração do
handler foi omitida sob um atributo de compilação condicional.

O passo a passo está dividido em seções e subseções, com uma lista numerada de
passos para cada divisão. Os códigos dados como exemplo não incluem os import
necessários para o funcionamento para facilitar a visualização da
implementação. Em caso de dúvida os imports podem ser checados nos arquivos
integrais. Vamos começar pelo backend.

## Backend

1. Criar o arquivo para colocar o novo aquisitor e declarar o seu módulo em
 acquisitors/mod.rs

> No nosso exemplo o aquisitor criado se chama 'example'.
> O módulo é declarado:
> ``` rust
> pub mod example;
> ```

2. Criar a estrutura do aquisitor e da configuração e os enums do estado e erros

> No nosso caso, não existem pontos de erro real, visto que o aquisitor
> independe de qualquer questão que possa trazer esses erros. Um foi incluído
> para ilustrar como devem ser adicionados. A biblioteca 'thiserror' traz uma
> forma simples de implementar o trait 'Error', que será necessário para
> implementar as funções do aquisitor mais pra frente.
``` rust
#[derive(Debug, thiserror::Error)]
pub enum ExampleErrors {
    #[error("Não conectou")]
    ConnectFail,
}
```

> Os estados são apenas os 3 que serão repassados para os do
> 'spectrum\_handler', exceto que o 'Connected' e 'Reading' guardam variáveis.
> O u32 do 'Connected' é apenas como exemplo, enquanto que o do 'Reading' tem
> uma função. A thread será criada de forma a depender da existência desse
> Sender. Assim, quando o estado for alterado, o objeto interno será desalocado
> e a thread irá terminar automaticamente. Eu recomendo que deixe sem variável
> a princípio, e defina ela quando for definir o funcionamento da thread.
``` rust
#[derive(Debug)]
pub enum ExampleState {
    Disconnected,
    Connected(u32),
    Reading(mpsc::Sender<ExampleConfig>),
}
```

> A configuração deve guardar todos os parâmetros do seu aquisitor que devem
> ser ajustáveis pelo usuário. No caso do nosso exemplo, iremos criar uma curva
> senoidal e enviar como se fosse um espectro de tempos em tempos. A
> configuração deve derivar dos traits da biblioteca 'serde' 'Serialize' e
> 'Deserialize' para que possam ser transferidas entre o back e o front end e
> para que possam ser salvas no sistema. O clone será útil para enviar pelo
> sender quando a thread de leitura for definida.
``` rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExampleConfig {
    points: u64,
    amplitude: f64,
    phase_t_speed: f64,
    phase_x_speed: f64,
    update_delay_millis: u64,
}
```

É importante que a estrutura do aquisitor mantenha posse do seu estado, de um
transmissor para o logger e da sua própria configuração. Os três devem ser
compartilháveis para poder enviar para a thread de leitura. O log\_sender pode
ser clonado para ser enviado para quantas threads quiser. O estado pode ser
alterado pela thread, então a mesma referência deve ser compartilhada através
de um Arc<Mutex>. A configuração pode ser enviada por cópia e posteriormente
por um canal. Ainda é útil manter ele dentro de um ponteiro para garantir a
mutabilidade interna para referências.

> No nosso exemplo apenas seguiremos o que foi recomendado acima para a
> estrutura do aquisitor:
``` rust
#[derive(Debug)]
pub struct Example {
    state: Arc<Mutex<ExampleState>>,
    pub log_sender: SyncSender<Log>,
    pub config: Mutex<ExampleConfig>,
}
```

3. Implementar o trait AcquisitorTrait de acquisitors/mod.rs na estrutura do
  aquisitor

O trait explica o que cada função deve executar. Note que o enum de erros deve
ser estático, não possuir variáveis armazenadas, e implementar o Trait do std
'Error' para poder ser parte do retorno. O clippy não gosta muito dessa ideia
do refino de Trait, mas você pode avisar que é intencional com a flag
'#![allow(refining_impl_trait)]' no início do arquivo.

> A explicação do Trait está como docstring. Replicarei aqui para facilitar:
```rust
...
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

...
```

> As implementações no exemplo são bem diretas e apenas seguem as orientações
> do trait. O 'disconnect' e 'stop\_reading' não precisaram de nenhum
> tratamento especial para matar a thread porque ela foi definida para depender
> do sender da configuração para se manter viva, como explicado anteriormente.
> Assim, ela é eliminada automaticamente quando o estado deixar de ser
> 'Reading'.
``` rust
impl AcquisitorTrait for Example {
    fn connect(&self) -> Result<(), ExampleErrors> {
        let mut state = self.state.lock().unwrap();

        if let ExampleState::Disconnected = *state {
            *state = ExampleState::Connected(3);
        } else {
            log_error(&self.log_sender, "[AEC] Não foi possível conectar. Aquisitor não está desconectado".into());
            return Err(ExampleErrors::ConnectFail);
        }

        log_info(&self.log_sender, "[AEC] Aquisitor de exemplo conectado".into());
        Ok(())
    }

    fn disconnect(&self) -> Result<(), ExampleErrors> {
        let mut state = self.state.lock().unwrap();

        if let ExampleState::Disconnected = *state {
            log_error(&self.log_sender, "[AEC] Não foi possível desconectar. Aquisitor já está desconectado".into());
            return Err(ExampleErrors::ConnectFail);
        } else {
            *state = ExampleState::Disconnected;
        }

        log_info(&self.log_sender, "[AEC] Aquisitor de exemplo desconectado".into());
        Ok(())
    }

    fn start_reading(&self, handler: &crate::spectrum_handler::SpectrumHandler, single_read: bool,) -> Result<(), ExampleErrors> {
        let acq_config = self.config.lock().unwrap();

        if single_read {
            let mut last_spectrum = handler.last_spectrum.lock().unwrap();
            *last_spectrum = Some(get_example_spectrum(&acq_config));
            handler.unread_spectrum.store(true, std::sync::atomic::Ordering::Relaxed);
            return Ok(());
        }

        let handler_config = handler.config.lock().unwrap();
        let mut state = self.state.lock().unwrap();

        let (config_tx, config_rx) = mpsc::channel();

        let args = ConstantReadArgs {
            last_spectrum: handler.last_spectrum.clone(),
            unread_spectrum_flag: handler.unread_spectrum.clone(),
            saving: handler.saving_new.clone(),
            auto_save_path: handler_config.auto_save_path.clone(),
            log_tx: self.log_sender.clone(),
            state: self.state.clone(),
            config_rx
        };

        let _ = config_tx.send((*acq_config).clone());
        *state = ExampleState::Reading(config_tx);

        thread::spawn(move || constant_read(args));

        log_info(&self.log_sender, "[AEC] Aquisitor de exemplo começou a ler".into());
        Ok(())
    }

    fn stop_reading(&self) -> Result<(), ExampleErrors> {
        let mut state = self.state.lock().unwrap();
        *state = ExampleState::Connected(9);

        log_info(&self.log_sender, "[AEC] Aquisitor de exemplo parou de ler".into());

        Ok(())
    }

    fn get_simplified_state(&self) -> crate::spectrum_handler::State {
        let state = self.state.lock().unwrap();

        match &*state {
            ExampleState::Disconnected => State::Disconnected,
            ExampleState::Connected(_) => State::Connected,
            ExampleState::Reading(_) => State::Reading,
        }
    }
}
```

> O start reading copia as referências e objetos necessários, acumula em uma
> estrutura para evitar erros ao passar os argumentos e inicia uma thread com
> uma função que realiza a leitura dos espectros continuamente. O
> start\_reading também permite apenas uma leitura no modo single\_read. Caso
> seja esse o caso ele apenas gera um espectro e imediatamente executa o
> processo de atualização descrito no trait. Podemos notar que ele busca o
> espectro de uma função geradora. Essa função é um ponto bem importante na
> implementação do novo aquisitor. No nosso caso é simples porque o foco desse
> tutorial é na integração do aquisitor. O que importa é que o espectro seja
> adquirido da fonte que está sendo implementada e formatado na forma da
> estrutura 'Spectrum'. No nosso caso a função gera uma senoidal.
``` rust
fn get_example_spectrum(config: &ExampleConfig) -> Spectrum {
    let time: u128 = time::SystemTime::now().duration_since(time::UNIX_EPOCH).unwrap().as_millis();
    let time = (time % 3_600_000) as f64;
    let time = time / 3600.0;

    let spec_vec = (0..config.points)
        .map(|x| x as f64)
        .map(|x| x / (config.points as f64))
        .map(|x| (x, time * config.phase_t_speed + x * config.phase_x_speed))
        .map(|(x, y)| (x, config.amplitude * y.cos()))
        .map(|(x, y)| SpectrumValue{wavelength: x*std::f64::consts::PI, power: y})
        .collect::<Vec<SpectrumValue>>();

    Spectrum::from_values(spec_vec)
}
```

> A thread de leitura contínua é iniciada em uma função que repete o processo
> da leitura única, exceto que ele adiciona um delay e uma busca por
> configurações entre cada leitura e confere se é para salvar automaticamente.
> A busca pela configuração é feita pelo 'Receiver' do canal que possui o
> objeto dentro do 'Reading' como 'Sender'. Caso o 'Receiver' receba o sinal de
> que o canal foi fechado ('Disconnected'), a função retorna, encerrando a
> thread. Em caso de ser pedido o salvamento, a função chama a função
> especificada no docstring do trait
``` rust
struct ConstantReadArgs {
     last_spectrum: Arc<Mutex<Option<Spectrum>>>,
     unread_spectrum_flag: Arc<AtomicBool>,
     saving: Arc<AtomicBool>,
     auto_save_path: PathBuf,
     log_tx: SyncSender<Log>,
     state: Arc<Mutex<ExampleState>>,
     config_rx: Receiver<ExampleConfig>,
}

fn constant_read(args: ConstantReadArgs) {
     let mut config = ExampleConfig {
         points: 0,
         amplitude: 0.0,
         phase_t_speed: 0.0,
         phase_x_speed: 0.0,
         update_delay_millis: 0,
     };

     loop {
         {   // This is to make sure the locks drop before the sleep
             match args.config_rx.try_recv() {
                 Ok(new_config) => {
                     config = new_config;
                 }
                 Err(TryRecvError::Empty) => (),
                 // Esse braço garante que a thread vai morrer assim que o estado for alterado, visto
                 // que o transmissor de configuração vai ser desalocado
                 Err(TryRecvError::Disconnected) => return,
             }

             let spectrum = get_example_spectrum(&config);

             if args.saving.load(std::sync::atomic::Ordering::Relaxed)
                 && auto_save_spectrum(&spectrum, &args.auto_save_path).is_err()
             {
                 log_war(&args.log_tx, "[AER] Falha ao salvar espectro automaticamente, interrompendo
                     leitura".into());

                 let mut state = args.state.lock().unwrap();
                 *state = ExampleState::Disconnected;
                 return;
             }

             let mut last_spectrum = args.last_spectrum.lock().unwrap();
             *last_spectrum = Some(spectrum);
             args.unread_spectrum_flag.store(true, std::sync::atomic::Ordering::Relaxed);
         }

         thread::sleep(Duration::from_millis(config.update_delay_millis));
     }

}
```

4. Implementar as funções de inicialização das estruturas de configuração e do
  aquisitor

A configuração padrão deve ter valores que sejam úteis como padrão. Eles não
serão valores temporários, a primeira configuração ao selecionar o aquisitor
virá dela.

> O passo mais complicado e verboso foi o anterior, a partir de agoras as
> coisas ficam bem diretas e boilerplate... No nosso exemplo:
``` rust
// Region: Default generators -------------------------------------------------

pub fn new_example(config: ExampleConfig, log_sender: SyncSender<Log>) -> Example {
    Example {
        state: Arc::new(Mutex::new(ExampleState::Disconnected)),
        log_sender,
        config: Mutex::new(config),
    }
}

pub fn default_config() -> ExampleConfig {
    ExampleConfig {
        points: 1024,
        amplitude: 2.0,
        phase_t_speed: 6.14,
        phase_x_speed: 6.14,
        update_delay_millis: 100,
    }
}
```

5. Implementar as funções referentes à configuração do aquisitor (update\_config
  e get\_config)

O update\_config deve ser capaz de enviar a configuração em tempo real, mesmo
se o aquisitor estiver em modo de leitura contínua

> No nosso exemplo, a configuração é atualizada diretamente no objeto da
> estrutura principal e enviada pelo sender caso o estado seja 'Reading'. O get
> config pega direto do objeto e clona para dar o retorno como Owned.
``` rust
impl Example {
    pub fn update_config(&self, new_config: ExampleConfig) {
        let mut config = self.config.lock().unwrap();
        let state = self.state.lock().unwrap();

        if let ExampleState::Reading(config_tx) = &*state {
            let _ = config_tx.send(new_config.clone());
        }

        *config = new_config;
    }

    pub fn get_config(&self) -> ExampleConfig {
        let config = self.config.lock().unwrap();

        (*config).clone()
    }
}
```

*Agora para incorporar nos locais fora do arquivo do próprio aquisitor:*

### spectrum\_handler/mod.rs
6. Atualizar os enums 'Acquisitor', 'AcquisitorSimple' e 'AcquisitorConfig'

``` rust
...
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum AcquisitorSimple {
    FileReader,
    Imon,
    Example // Essa linha a mais
}

#[derive(Debug)]
pub enum Acquisitor {
    FileReader(FileReader),
    Imon(Imon),
    Example(Example) // E essa
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum AcquisitorConfig {
    FileReaderConfig(FileReaderConfig),
    ImonConfig(ImonConfig),
    ExampleConfig(ExampleConfig) // E essa
}
...
```

Depois de fazer essa atualização todas as partes do código que dependem de um
match sobre um desses enum se tornarão erradas. Isso é útil para saltarmos
usando o lsp para as próximas atualizações a se fazer

7. Implementar os matches da interface com o aquisitor ('impl SpectrumHandler'),
  referentes às funções definidas no trait

> Essa parte é só copiar e colar, e vai seguir o mesmo padrão em todas as
> funções. Basta pegar a chamada feita para os braços anteriores e chamar para
> o seu novo aquisitor, como em:
``` rust
...
impl SpectrumHandler {
...
    pub fn connect(&self) -> Result<(), Box<dyn Error>> {
        let acquisitor = self.acquisitor.lock().unwrap();

        match &*acquisitor {
            Acquisitor::FileReader(file_reader) => file_reader.connect()?,
            Acquisitor::Imon(imon) => imon.connect()?,
            Acquisitor::Example(example) => example.connect()?, // Como nessa linha aqui
        }

        Ok(())
    }
...
}
...
```

8. Implementar os matches referentes à interação com a configuração ('impl
      SpectrumHandler' também, mas em outro ponto)

> Mesma ideia aqui, basta copiar a implementação dos outros braços
``` rust
...
impl SpectrumHandler {
...
    pub fn update_acquisitor_config(&self, new_config: AcquisitorConfig) {
        let acquisitor = self.acquisitor.lock().unwrap();

        match &*acquisitor {
            Acquisitor::FileReader(file_reader) => {
                if let AcquisitorConfig::FileReaderConfig(new_config) = new_config {
                    file_reader.update_config(new_config);
                } else {
                    self.log_error(
                        "[HUQ] Configuração incompatível, era esperado \
                        receber FileReaderConfig"
                            .to_string(),
                    );
                }
            }
            ... // A do IMON foi omitida para facilitar a visualização
            Acquisitor::Example(example) => { // Integra a configuração do novo aqui
                if let AcquisitorConfig::ExampleConfig(new_config) = new_config {
                    example.update_config(new_config);
                } else {
                    self.log_error(
                        "[HUQ] Configuração incompatível, era esperado \
                        receber ExampleConfig"
                            .to_string(),
                    );
                }
            }
        }
    }

    pub fn get_acquisitor_config(&self) -> AcquisitorConfig {
        let acquisitor = self.acquisitor.lock().unwrap();

        match &*acquisitor {
            Acquisitor::FileReader(file_reader) => {
                AcquisitorConfig::FileReaderConfig(file_reader.get_config())
            }
            Acquisitor::Imon(imon) => AcquisitorConfig::ImonConfig(imon.get_config()),
            Acquisitor::Example(example) => AcquisitorConfig::ExampleConfig(example.get_config()), // E aqui
        }
    }
...
}
...
```

### Acquisitors/mod.rs
9. Atualizar o match do 'load_acquisitor'

> Mesma ideia do copiar e colar um braço do match dos outros.
``` rust
...
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
        ...
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
...
```

> Iremos implementar o load_example_config em um momento, deixa ela em aberto por
ora

### config.rs
10. Implementar as funções referentes à leitura e escrita da configuração no
  sistema. Elas estão em uma subregião, como exemplo: 'Subregion: example
  config'. Copia as funções e ajuste elas para o seu novo aquisitor

> A busca pelas configurações também deve ser bem parecida com os outros,
> então é fácil de copiar e colar, alterando nomes no caminho. A do exemplo foi
> construída dessa forma

``` rust
// Subregion: example config --------------------------------------------------

pub fn example_config_path() -> PathBuf {
    let home = match home_dir() {
        Some(path) => path,
        None => Path::new("./").to_path_buf(), // If can't find home, uses config on pwd
    };

    home.join(".config/rosav/example_acq.toml")
}

pub fn load_example_config() -> Result<ExampleConfig, Box<dyn Error>> {
    let text = read_to_string(example_config_path())?;
    let config: ExampleConfig = toml::from_str(&text)?;

    Ok(config)
}

pub fn write_example_config(config: &ExampleConfig) -> Result<(), Box<dyn Error>> {
    let config_path = example_config_path();

    if let Some(parent) = config_path.parent() {
        // Enforces the parent folder
        create_dir_all(parent)?;
    }
    write(&config_path, toml::to_string(config)?)?;

    Ok(())
}
```
11. Atualizar os matches do 'load\_acquisitor\_config' e do
  'write\acquisitor\_config'

> mesmo esquema
``` rust
pub fn load_acquisitor_config(
    acquisitor_type: AcquisitorSimple,
) -> Result<AcquisitorConfig, Box<dyn Error>> {
    match acquisitor_type {
        AcquisitorSimple::FileReader => Ok(AcquisitorConfig::FileReaderConfig(
            load_file_reader_config()?,
        )),
        AcquisitorSimple::Imon => Ok(AcquisitorConfig::ImonConfig(load_imon_config()?)),
        AcquisitorSimple::Example => Ok(AcquisitorConfig::ExampleConfig(load_example_config()?)), // ++
    }
}

pub fn write_acquisitor_config(config: &AcquisitorConfig) -> Result<(), Box<dyn Error>> {
    match config {
        AcquisitorConfig::FileReaderConfig(config) => write_file_reader_config(config),
        AcquisitorConfig::ImonConfig(config) => write_imon_config(config),
        AcquisitorConfig::ExampleConfig(config) => write_example_config(config), // ++
    }
}
```

## Frontend

O próximo passo é atualizar as partes do frontend. Aqui as partes são bem
diretas. A parte que dá mais trabalho é a criação da seção de configuração.
Mesmo ela deve normalmente ser simples, visto que as já existentes oferecem um
bom padrão para comparação.

1. Criar a estrutura da configuração e a função da estrutura vazia em
  'api/acquisitors.rs'

Na mesma ideia da atualização dos enums no backend, depois de atualizar esse o
lsp vai dar erros nos próximos lugares a serem atualizados.

> Aqui vamos basicamente copiar as estruturas que já criamos. É importante que
> sejam idênticas, inclusive em nomes, para que a serialização e
> desserialização funcionem junto.
```rust
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
```

2. Incluir a nova configuração e o aquisitor simples nos enums do mesmo arquivo

> Mesma ideia do backend
```rust
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum AcquisitorSimple {
    FileReader,
    Imon,
    Example // ++
}

#[derive(Debug, Serialize, Deserialize, Clone)]
pub enum AcquisitorConfig {
    FileReaderConfig(FileReaderConfig),
    ImonConfig(ImonConfig),
    ExampleConfig(ExampleConfig), // ++
}
```

- Incluir a função de renderização da config do novo aquisitor em
  'side\_bar/acquisitor\_config\_renders.rs'

> Essa é a etapa que pode dar mais trabalho. É algo grande mas é bem seguir
> fórmula. Os comentários explicam o porque de cada parte antes da
> renderização. É bom que você tenha alguma prática com o Sycamore para
> entender a função também

```rust
#[component]
pub fn RenderExampleConfig<G: Html>(cx: Scope) -> View<G> {
    // Criar sinais
    let config = create_signal(cx, empty_example_config());

    let points = create_signal(cx, String::new());
    let amplitude = create_signal(cx, String::new());
    let phase_t_speed = create_signal(cx, String::new());
    let phase_x_speed = create_signal(cx, String::new());
    let update_delay_millis = create_signal(cx, String::new());

    // Ao iniciar, tenta pegar a configuração já existente
    spawn_local_scoped(cx, async move {
        // Tenta algumas vezes em caso de erros ou falta de sincronia
        for _ in 0..3 {
            let _config = get_acquisitor_config().await;

            if let AcquisitorConfig::ExampleConfig(_config) = _config {
                points.set(_config.points.to_string());
                amplitude.set(_config.amplitude.to_string());
                phase_t_speed.set(_config.phase_t_speed.to_string());
                phase_x_speed.set(_config.phase_x_speed.to_string());
                update_delay_millis.set(_config.update_delay_millis.to_string());

                config.set(_config);
                return;
            }
        }
    });

    // Cria o efeito para enviar a configuração para o backend sempre que o
    // sinal de config daqui for atualizado
    create_effect(cx, move || {
        config.track();
        spawn_local_scoped(cx, async move {
            if *config.get() != empty_example_config() {
                apply_acquisitor_config(AcquisitorConfig::ExampleConfig((*config.get()).clone()))
                    .await;
            }
        });
    });

    // Callback para quando algum campo for atualizado. Confere se é válido e,
    // para cada campo válido, atualiza o sinal de config. Se for atualizado
    // mais de uma vez só um sinal vai sair pro efeito acima, quando o
    // config.modify() for desalocado
    let update_config = |event: rt::Event| {
        event.prevent_default();

        let mut config = config.modify();

        match (*points.get()).parse::<u64>() {
            Ok(value) => config.points = value,
            Err(_) => points.set(config.points.to_string()),
        }

        match (*amplitude.get()).parse::<f64>() {
            Ok(value) => config.amplitude = value,
            _ => amplitude.set(config.amplitude.to_string()),
        }

        match (*phase_t_speed.get()).parse::<f64>() {
            Ok(value) => config.phase_t_speed = value,
            _ => phase_t_speed.set(config.phase_t_speed.to_string()),
        }

        match (*phase_x_speed.get()).parse::<f64>() {
            Ok(value) => config.phase_x_speed = value,
            _ => phase_x_speed.set(config.phase_x_speed.to_string()),
        }

        match (*update_delay_millis.get()).parse::<u64>() {
            Ok(value) => config.update_delay_millis = value,
            Err(_) => update_delay_millis.set(config.update_delay_millis.to_string()),
        }
    };

    // Renderiza a parte visual, esse padrão de div.elements segurando um 'p',
    // um input e (opcional) uma string a mais é o modelo que o css foi feito
    // para suportar. Se for útil fazer diferente, pode ser necessário mexer lá
    // também para deixar bonito
    view! { cx,
        form(on:submit=form_blur) {
            input(type="submit", style="display: none;")

            p(class="mini-title") {
                p { "Aquisitor" }
                p { "(Example) "}
            }

            div(class="element") {
                p { "Densidade: " }
                input(
                    bind:value=points,
                    type="number",
                    on:focusout=update_config
                ) {}
                "pontos"
            }

            div(class="element") {
                p { "Amplitude: " }
                input(
                    bind:value=amplitude,
                    on:input=|_| check_number_input(amplitude),
                    on:focusout=update_config
                ) {}
            }

            div(class="element") {
                p { "Velocidade angular em t: " }
                input(
                    bind:value=phase_t_speed,
                    on:input=|_| check_number_input(phase_t_speed),
                    on:focusout=update_config
                ) {}
                "rad/s"
            }

            div(class="element") {
                p { "Velocidade angular em x: " }
                input(
                    bind:value=phase_x_speed,
                    on:input=|_| check_number_input(phase_x_speed),
                    on:focusout=update_config
                ) {}
                "rad/un"
            }

            div(class="element") {
                p { "Delay entre leituras: " }
                input(
                    bind:value=update_delay_millis,
                    type="number",
                    on:focusout=update_config
                ) {}
                "ms"
            }
        }
    }
}
```

3. Atualizar os matchs que se tornaram inválidos com a adição dos novos enums
    - 'side\_bar/mod.rs' -> 'RenderAcquisitorConfig'
    - 'side\_bar/mod.rs' -> 'get_old_handler_config'

> Aqui é só seguir onde tem esses matches e adicionar o braço, bem direto
```rust

...

#[component]
fn RenderHandlerConfig<'a, G: Html>(cx: Scope<'a>, props: HandlerConfigProps<'a>) -> View<G> {
    ...
    let acquisitor_select = move |_| {
        blur();
        match (*acquisitor.get()).as_str() {
            "file_reader" => (props.config.modify()).acquisitor = AcquisitorSimple::FileReader,
            "imon" => (props.config.modify()).acquisitor = AcquisitorSimple::Imon,
            "example" => (props.config.modify()).acquisitor = AcquisitorSimple::Example, // ++
            _ => (),
        }
    };

    ...

}

...

async fn get_old_handler_config(signals: OldHandlerSignals<'_>) -> HandlerConfig {

    ...

    match _config.acquisitor {
        AcquisitorSimple::FileReader => signals.acquisitor.set("file_reader".to_string()),
        AcquisitorSimple::Imon => signals.acquisitor.set("imon".to_string()),
        AcquisitorSimple::Example => signals.acquisitor.set("example".to_string()), // ++
    }
}
```

4. Atualizar as opções no final da função de renderização da configuração
  'RenderHandlerConfig'

5. Atualizar o 'acquisitor\_select' dentro da mesma função

> Novamente, bem direto, é só seguir o modelo já imposto pelos outros
> aquisitores.
```rust

...

#[component]
fn RenderHandlerConfig<'a, G: Html>(cx: Scope<'a>, props: HandlerConfigProps<'a>) -> View<G> {

    ...

    let acquisitor_select = move |_| {
        blur();
        match (*acquisitor.get()).as_str() {
            "file_reader" => (props.config.modify()).acquisitor = AcquisitorSimple::FileReader,
            "imon" => (props.config.modify()).acquisitor = AcquisitorSimple::Imon,
            "example" => (props.config.modify()).acquisitor = AcquisitorSimple::Example, // ++
            _ => (),
        }
    };

    ...

    view! { cx,
        form(class="side-container back config", on:submit=unfocus) {
            ...
            div(class="element") {
                p { "Tipo de aquisitor:" }
                select(
                    name="acquisitor",
                    bind:value=acquisitor,
                    on:input=acquisitor_select
                ) {
                    option(value="file_reader") { "Leitor de arquivos" }
                    option(value="imon") { "Ibsen IMON" }
                    option(value="example") { "Aquisitor exemplo" } // ++
                    )
                }
            }
        }
    }
}

...
```

Depois de fazer tudo isso, o programa deve compilar com o seu novo aquisitor!
