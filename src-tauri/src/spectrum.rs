#![allow(dead_code)]

#[derive(Debug)]
struct SpectrumValue {
    wavelength: f64,
    power: f64
}

#[derive(Debug)]
pub struct Spectrum {
    values: Vec<SpectrumValue>
}
