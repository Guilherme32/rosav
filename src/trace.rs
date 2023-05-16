
#[derive(Clone, PartialEq, Debug)]
pub struct Trace {
    pub id: u8,
    pub visible: bool,
    pub draw_valleys: bool,                // TODO adicionar detecção de vale
    pub active: bool,
    pub valleys: Vec<f64>,
    pub svg_size: (i32, i32),
    pub svg_path: String,
    pub freeze_time: Option<String>        // Se None não está congelado
}

pub fn new_trace(id: u8) -> Trace {
    Trace {
        id,
        visible: true,
        draw_valleys: true,
        active: true,
        valleys: vec![],
        svg_size: (0, 0),
        svg_path: String::new(),
        freeze_time: None
    }
}

pub fn trace_id_to_name(id: u8) -> String {
    if id > 25 {
        format!("{}", id)
    } else {
        let letters = vec!["A", "B", "C", "D", "E", "F", "G", "H", "I", "J",
                           "K", "L", "M", "N", "O", "P", "Q", "R", "S", "T",
                           "U", "V", "W", "X", "Y", "Z"];
        format!("{}", letters[id as usize])
    }
}

pub fn trace_id_to_color(id: u8) -> String {
    // TODO passar essa função pro backend e pegar de um arquivo de configuração
    if id > 8 {
        trace_id_to_color(id - 9)
    } else {
         let colors = vec!["#ce1417", "#377eb8", "#4daf4a", "#984ea3",
                           "#ff7f00", "#a0a006", "#a65628", "#f781bf",
                           "#999999"];
        format!("{}", colors[id as usize])
    }
}

pub fn trace_id_to_style(id: u8) -> String {
    format!("background-color: {};", trace_id_to_color(id))
}

