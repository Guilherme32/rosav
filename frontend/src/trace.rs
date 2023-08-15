use sycamore::prelude::*;

use crate::api::CriticalDetection;

#[derive(Clone, PartialEq, Debug)]
pub struct TraceInfo {
    pub wavelength_limits: (f64, f64),
    pub power_limits: (f64, f64),
    pub svg_size: (i32, i32),
    pub valley_detection: CriticalDetection,
    pub peak_detection: CriticalDetection,
}

pub fn empty_trace_info() -> TraceInfo {
    TraceInfo {
        wavelength_limits: (0.0, 0.0),
        power_limits: (0.0, 0.0),
        svg_size: (0, 0),
        valley_detection: CriticalDetection::None,
        peak_detection: CriticalDetection::None,
    }
}

#[derive(Clone, PartialEq, Debug)]
pub struct Trace {
    pub id: u8,
    pub visible: bool,
    pub draw_valleys: bool,
    pub draw_valleys_mean: bool,
    pub color_id: Option<u8>,
    pub active: bool,
    pub valleys: Vec<(f64, f64)>,
    pub peaks: Vec<(f64, f64)>,
    pub svg_path: String,
    pub freeze_time: Option<String>, // Se None não está congelado
    pub drawn_info: TraceInfo,       // Stuff to check if it needs to be redrawn
}

pub fn new_trace(id: u8, visible: bool, draw_valleys: bool, draw_valleys_mean: bool) -> Trace {
    Trace {
        id,
        visible,
        draw_valleys,
        draw_valleys_mean,
        color_id: None,
        active: true,
        valleys: vec![],
        peaks: vec![],
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

static COLORS: &[&str] = &[
    // rYellow   cBlue      sRed       oViolet
    "#ff9e3b", "#7e9cd8", "#e82424", "#957fb8",
    // wAqua      sPink      aGreen     kGray   sOrange
    "#7aa89f", "#d27e99", "#76946a", "#717c7c", "#ffa066",
];

fn trace_id_to_color(id: u8) -> String {
    let id = (id as usize) % COLORS.len();
    COLORS[id].to_string()
}

impl Trace {
    pub fn get_color(&self) -> String {
        if let Some(color_id) = self.color_id {
            trace_id_to_color(color_id)
        } else {
            trace_id_to_color(self.id)
        }
    }

    pub fn style(&self) -> String {
        format!("background-color: {};", self.get_color())
    }

    pub fn change_color(&mut self) {
        if let Some(color_id) = self.color_id {
            self.color_id = Some((color_id + 1) % (COLORS.len() as u8));
        } else {
            self.color_id = Some((self.id + 1) % (COLORS.len() as u8));
        }
    }

    pub fn valleys_mean(&self) -> Option<(f64, f64)> {
        if self.valleys.len() < 2 {
            return None;
        }

        let sum = self
            .valleys
            .iter()
            .fold((0.0, 0.0), |acc, new| (acc.0 + new.0, acc.1 + new.1));

        let len = self.valleys.len();
        Some((sum.0 / (len as f64), sum.1 / (len as f64)))
    }

    pub fn peaks_mean(&self) -> Option<(f64, f64)> {
        if self.peaks.len() < 2 {
            return None;
        }

        let sum = self
            .peaks
            .iter()
            .fold((0.0, 0.0), |acc, new| (acc.0 + new.0, acc.1 + new.1));

        let len = self.peaks.len();
        Some((sum.0 / (len as f64), sum.1 / (len as f64)))
    }
}

// Drawing implementations
impl Trace {
    pub fn render_spectrum<G: Html>(self, cx: Scope) -> View<G> {
        let color = self.get_color();

        if self.visible {
            view! { cx,
                path(
                    d=self.svg_path,
                    fill="none",
                    stroke-width="2",
                    stroke=color,
                    clip-path="url(#graph-clip)"
                ) {}
            }
        } else {
            view! { cx, "" }
        }
    }

    pub fn render_valleys_markers<G: Html>(&self, cx: Scope) -> View<G> {
        let color = self.get_color();

        if self.draw_valleys {
            View::new_fragment(
                self.valleys
                    .iter()
                    .map(|&valley| {
                        let color = color.clone();
                        view! { cx,
                            circle(
                                cx=valley.0,
                                cy=valley.1,
                                r="6",
                                stroke-width="2",
                                stroke="#16161D",
                                fill=color,
                                clip-path="url(#graph-clip)"
                            ) {}
                            line(
                                x1=valley.0,
                                x2=valley.0,
                                y1=(valley.1 + 3.0),
                                y2=(valley.1 - 3.0),
                                stroke-width="2",
                                stroke="#16161D",
                                clip-path="url(#graph-clip)"
                            ) {}
                        }
                    })
                    .collect(),
            )
        } else {
            view! { cx, "" }
        }
    }

    pub fn render_peaks_markers<G: Html>(&self, cx: Scope) -> View<G> {
        let color = self.get_color();

        if self.draw_valleys {
            View::new_fragment(
                self.peaks
                    .iter()
                    .map(|&peak| {
                        let color = color.clone();
                        view! { cx,
                            circle(
                                cx=peak.0,
                                cy=peak.1,
                                r="6",
                                stroke-width="2",
                                stroke="#16161D",
                                fill=color,
                                clip-path="url(#graph-clip)"
                            ) {}
                            line(
                                x1=(peak.0 + 0.707 * 3.0), // 3.0*sqrt(2) to get the same length
                                x2=(peak.0 - 0.707 * 3.0),
                                y1=(peak.1 + 0.707 * 3.0),
                                y2=(peak.1 - 0.707 * 3.0),
                                stroke-width="2",
                                stroke="#16161D",
                                clip-path="url(#graph-clip)"
                            ) {}
                        }
                    })
                    .collect(),
            )
        } else {
            view! { cx, "" }
        }
    }

    pub fn render_valleys_mean_marker<G: Html>(&self, cx: Scope) -> View<G> {
        let color = self.get_color();
        if self.draw_valleys_mean {
            if let Some(valleys_mean) = self.valleys_mean() {
                view! { cx,
                    circle(
                        cx=valleys_mean.0,
                        cy=valleys_mean.1,
                        r="6",
                        stroke-width="2",
                        stroke="#16161D",
                        fill=color,
                        clip-path="url(#graph-clip)"
                    ) {}
                    line(
                        x1=valleys_mean.0,
                        x2=valleys_mean.0,
                        y1=(valleys_mean.1 + 3.0),
                        y2=(valleys_mean.1 - 3.0),
                        stroke-width="2",
                        stroke="#16161D",
                        clip-path="url(#graph-clip)"
                    ) {}
                    line(
                        x1=(valleys_mean.0 + 3.0),
                        x2=(valleys_mean.0 - 3.0),
                        y1=valleys_mean.1,
                        y2=valleys_mean.1,
                        stroke-width="2",
                        stroke="#16161D",
                        clip-path="url(#graph-clip)"
                    ) {}
                }
            } else {
                view! { cx, "" }
            }
        } else {
            view! { cx, "" }
        }
    }

    pub fn render_peaks_mean_marker<G: Html>(&self, cx: Scope) -> View<G> {
        let color = self.get_color();
        if self.draw_valleys_mean {
            if let Some(peaks_mean) = self.peaks_mean() {
                view! { cx,
                    circle(
                        cx=peaks_mean.0,
                        cy=peaks_mean.1,
                        r="6",
                        stroke-width="2",
                        stroke="#16161D",
                        fill=color,
                        clip-path="url(#graph-clip)"
                    ) {}
                    line(
                        x1=(peaks_mean.0 - 0.707 * 3.0), // 3.0*sqrt(2) to get the same length
                        x2=(peaks_mean.0 + 0.707 * 3.0),
                        y1=(peaks_mean.1 + 0.707 * 3.0),
                        y2=(peaks_mean.1 - 0.707 * 3.0),
                        stroke-width="2",
                        stroke="#16161D",
                        clip-path="url(#graph-clip)"
                    ) {}
                    line(
                        x1=(peaks_mean.0 + 0.707 * 3.0), // 3.0*sqrt(2) to get the same length
                        x2=(peaks_mean.0 - 0.707 * 3.0),
                        y1=(peaks_mean.1 + 0.707 * 3.0),
                        y2=(peaks_mean.1 - 0.707 * 3.0),
                        stroke-width="2",
                        stroke="#16161D",
                        clip-path="url(#graph-clip)"
                    ) {}
                }
            } else {
                view! { cx, "" }
            }
        } else {
            view! { cx, "" }
        }
    }
}
