#[derive(Clone, PartialEq, Debug)]
pub struct TraceInfo {
    pub wavelength_limits: (f64, f64),
    pub power_limits: (f64, f64),
    pub svg_size: (i32, i32)
}

fn empty_trace_info() -> TraceInfo {
    TraceInfo {
        wavelength_limits: (0.0, 0.0),
        power_limits: (0.0, 0.0),
        svg_size: (0, 0)
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Trace {
    pub id: u8,
    pub visible: bool,
    pub draw_valleys: bool,                // TODO adicionar detecção de vale
    pub active: bool,
    pub valleys: Vec<f64>,
    // pub svg_size: (i32, i32),
    pub svg_path: String,
    pub freeze_time: Option<String>,        // Se None não está congelado
    pub drawn_info: TraceInfo             // Stuff to check if it needs to be redrawn
}

pub fn new_trace(id: u8) -> Trace {
    Trace {
        id,
        visible: true,
        draw_valleys: true,
        active: true,
        valleys: vec![],
        svg_path: String::new(),
        freeze_time: None,
        drawn_info: empty_trace_info()
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
                        // rYellow   dBlue      sRed       oViolet
         let colors = vec!["#ff9e3b", "#658594", "#e82424", "#957fb8",
                        // wAqua      sPink      aGreen     kGray
                           "#7aa89f", "#d27e99", "#76946a", "#717c7c",
                        // cBlue
                           "#7e9cd8"];
        format!("{}", colors[id as usize])
    }
}

pub fn trace_id_to_style(id: u8) -> String {
    format!("background-color: {};", trace_id_to_color(id))
}

