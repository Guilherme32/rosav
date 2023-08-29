# RosaV - Reliable OSA Visualizer

Um visualizador para analisadores de espectro óptico (OSA) escrito em
rust. Foi criado com na Universidade Federal de Juiz de Fora (UFJF), no
laboratório de instrumentação e telemetria (LiTel). O objetivo principal do
desenvolvimento do programa é ter uma opção de interface confiável, capaz de
comunicar com diferentes instrumentos, e com funcionalidades ergonômicas para o
desenvolvimento, fabricação e análise de sensores de LPG em fibra.

EN: A visualizer for optical spectrum analizers (OSA) written in rust. It was
created at the Federal University of Juiz de Fora (UFJF), at the laboratory of
instrumentation and telemetry (LiTel). The main objective for the development
of the application is to have an option of interface that is reliable, capable
of communicating with different instruments, and with ergonomic features for
the development, fabrication and analisys of LPG fiber sensors. Since it was
made to be used at the lab, in Brazil, the main user side documentation (readme
and manual) is written in portuguese. If this project may be of use to you,
and you do not speak portuguese, please send me an e-mail, and I will consider
translating it.

# OSAs com suporte

- IBSEN IMON 512 / 256 (testado no 521, mas deve funcionar para o 256 também)

# Como usar

Para usuários de windows, os arquivos binários pré compilados podem ser
encontrados na seção de releases do repositório no github. Os releases marcados
como draft são gerados automaticamente por actions, seguindo a última versão do
código disponível. Para instalar, execute o arquivo .msi e siga as instruções. O
programa não possui um certificação da Microsoft, então o computador irá acusar
a possibilidade de malware. Ignore o aviso e peça para instalar mesmo assim. Uma
vez instalado, basta executar o rosav.exe (ou o atalho gerado).

Para usuários de outras plataformas, é necessária a compilação no próprio
sistema. O processo não é tão direto, e pode ser demorado na primeira
compilação. Para compilar localmente, siga primeiro o processo de instalação
de dependências descrito para o desenvolvimento. Com todas as ferramentas
instaladas, vá no diretório 'src-tauri' e execute:

`cargo tauri build`

Se tudo correr como o esperado, o próprio comando irá responder com a
localização dos binários gerados, no formato utilizado pelo seu sistema.

Um manual será em breve elaborado, explicando como utilizar todas as
funcionalidades do programa. Por enquanto, a melhor forma de aprender a utilizar
é pedindo ajuda a alguém que já sabe.

# Desenvolvimento

Para poder colaborar com código para o projeto, uma série de ferramentas devem
ser instaladas. A primeira, e mais importante, é a toolchain de desenvolvimento
para rust. A recomendação é instalar pelo rustup:

[](https://rustup.rs/)

O projeto foi desenvolvido utilizando tauri. Tauri é um framework
de rust para a criação de aplicativos multi-plataforma, eficientes e seguros,
com a utilização de ferramentas web para a construção da interface visual. Isso
facilita uma separação de responsabilidades entre o back e o front end, além
de permitir o uso do vasto ecossistema web já existente. Programas utilizando
tauri não são compilados da forma tradicional em rust, diretamente pelo cargo,
mas sim por um conjunto de comandos próprios. Para rodar esses comandos, é
necessário instalar o tauri cli. Isso pode ser feito pelo cargo com:

`cargo install tauri-cli`

Por último, a base do front end é a biblioteca sycamore, que permite a construção
de aplicações web com rust. Para realizar a compilação, é necessário adicionar
o compilador cruzado para WebAssembly:

`rustup target add wasm32-unknown-unknown`

Também é necessário instalar uma ferramenta para compilar, fazer o bundle, e
servir os binários web. Uma ferramenta que faz exatamente isso é o trunk, que
pode ser instalado também com o cargo:

`cargo install trunk`

Com isso, as ferramentas necessárias foram todas instaladas. Para executar a
versão de teste do programa, vá no diretório 'src-tauri' e execute:

`cargo tauri dev`

A primeira compilação é demorada, mas as seguintes são rápidas. Depois de
compilar tudo, o programa será aberto. Tabmém, tanto o trunk quanto o tauri
funcionam com recompilação automática com alteração dos arquivos, então não
é necessário fechar a aplicação e re-executar o comando toda vez que mudar
o código.

## Anotações adicionais

Eu imagino que existe a possibilidade de alguém receber a tarefa de atualizar
esse código em um tempo futuro em que não estou mais presente no laboratório.
Imagino também que, dentro dessa possibilidade, existe a de que esse alguém
não é familiar, ou não conhece as ferramentas aqui utilizadas. Para ajudar esse
alguém no casso dessa possibilidade, deixo aqui o caminho que eu recomendaria
seguir antes de realmente começar a mexer no código.

* Para o caminho, eu estou assumindo que esse alguém já possui experiência /
contato com programação

- Aprender rust: A linguagem é um pouco mais difícil de aprender e utilizar do
que python ou C/C++, mas as suas vantagens facilmente compensam esse fato. Eu
recomendo seguir o livro do rust na íntegra, realizando todos os exercícios
propostos.

[](https://doc.rust-lang.org/stable/book/)

- Aprender o básico de web: Esse passo é um que acredito ter maior chance de ser
desnecessário, porque as tecnologias web são populares, e alguém com contato com
programação tem uma grande chance de também ter contato com HTML/CSS/JS. Se não
tiver, recomendo procurar algum recurso básico que ensine essas três ferramentas
básicas antes de tentar entender os frameworks utilizados. Um recurso muito bem
elaborado é o do curso CS50 de Harvard. Ele é maior do que um tutorial, mas é
capaz de ensinar não apenas a escrever os códigos, mas a entender as ferramentas
utilizadas. Ao seguir as aulas e fazer os projetos propostos, o conhecimento
recebido é muito mais profundo do que aquele recebido de tutoriais simples.

[](https://cs50.harvard.edu/web/2020/)

- Aprender tauri e sycamore: Depois que já souber rust, eu recomendo aprender
o básico dos frameworks utilizados. Aqui não precisa ir muito fundo, mas ainda
assim acredito que poderá ser de grande utilidade seguir os capítulos básicos
dos seus livros.

[](https://sycamore-rs.netlify.app/docs/v0.8/getting_started/installation)
[](https://tauri.app/v1/guides/getting-started/prerequisites)