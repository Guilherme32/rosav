use crate::api::ValleyDetection;

#[derive(Clone, PartialEq, Debug)]
pub struct TraceInfo {
    pub wavelength_limits: (f64, f64),
    pub power_limits: (f64, f64),
    pub svg_size: (i32, i32),
    pub valley_detection: ValleyDetection,
}

fn empty_trace_info() -> TraceInfo {
    TraceInfo {
        wavelength_limits: (0.0, 0.0),
        power_limits: (0.0, 0.0),
        svg_size: (0, 0),
        valley_detection: ValleyDetection::None,
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Trace {
    pub id: u8,
    pub visible: bool,
    pub draw_valleys: bool, // TODO adicionar detecção de vale
    pub active: bool,
    pub valleys: Vec<(f64, f64)>,
    pub svg_path: String,
    pub freeze_time: Option<String>, // Se None não está congelado
    pub drawn_info: TraceInfo,       // Stuff to check if it needs to be redrawn
}

pub fn new_trace(id: u8, visible: bool, draw_valleys: bool) -> Trace {
    Trace {
        id,
        visible,
        draw_valleys,
        active: true,
        valleys: vec![],
        svg_path: String::new(),
        freeze_time: None,
        drawn_info: empty_trace_info(),
    }
}

static LETTERS: &[&str] = &[
    "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S",
    "T", "U", "V", "W", "X", "Y", "Z",
];

pub fn trace_id_to_name(id: u8) -> String {
    if (id as usize) >= LETTERS.len() {
        format!("{}", id)
    } else {
        LETTERS[id as usize].to_string()
    }
}

pub fn trace_id_to_color(id: u8) -> String {
    if id > 8 {
        trace_id_to_color(id - 9)
    } else {
        // rYellow   cBlue      sRed       oViolet
        let colors = vec![
            "#ff9e3b", "#7e9cd8", "#e82424", "#957fb8",
            // wAqua      sPink      aGreen     kGray
            "#7aa89f", "#d27e99", "#76946a", "#717c7c", // sOrange
            "#ffa066",
        ];
        colors[id as usize].to_string()
    }
}

pub fn trace_id_to_style(id: u8) -> String {
    format!("background-color: {};", trace_id_to_color(id))
}
