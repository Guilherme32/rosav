#![allow(dead_code)]

use csv;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt;
use std::path::Path;

use find_peaks::PeakFinder;
use itertools::Itertools;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SpectrumValue {
    pub wavelength: f64,
    pub power: f64,
}

#[derive(Debug, Clone)]
pub struct Spectrum {
    pub values: Vec<SpectrumValue>,
    pub limits: Limits,
    pub valleys: Option<Vec<SpectrumValue>>,
}

#[derive(Debug, Clone)]
pub struct Limits {
    pub wavelength: (f64, f64),
    pub power: (f64, f64),
}

impl fmt::Display for Spectrum {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for value in &self.values {
            writeln!(f, "({:.4e}, {:.4e})", value.wavelength, value.power)?;
        }
        Ok(())
    }
}

fn convert_point(
    graph_limits: &Limits,
    svg_limits: &(f64, f64),
    og_point: &SpectrumValue,
) -> (f64, f64) {
    let limits_pwr = (graph_limits.power.1, graph_limits.power.0); // Invert because svg coords
    let limits_wl = graph_limits.wavelength;

    let x = (og_point.wavelength - limits_wl.0) / (limits_wl.1 - limits_wl.0);
    let x = x * svg_limits.0;

    let y = (og_point.power - limits_pwr.0) / (limits_pwr.1 - limits_pwr.0);
    let y = y * svg_limits.1;

    (x, y)
}

fn bezier_point(
    previous: (f64, f64),
    start: (f64, f64),
    end: (f64, f64),
    next: (f64, f64),
) -> String {
    let smoothing = 0.3;

    let start_vector = (end.0 - previous.0, end.1 - previous.1);
    let start_control = (
        start.0 + start_vector.0 * smoothing,
        start.1 + start_vector.1 * smoothing,
    );

    let end_vector = (start.0 - next.0, start.1 - next.1);
    let end_control = (
        end.0 + end_vector.0 * smoothing,
        end.1 + end_vector.1 * smoothing,
    );

    format!(
        "C {:.2},{:.2} {:.2},{:.2}, {:.2},{:.2} ",
        start_control.0, start_control.1, end_control.0, end_control.1, end.0, end.1
    )
}

impl Spectrum {
    pub fn empty() -> Spectrum {
        Spectrum {
            values: vec![],
            valleys: None,
            limits: Limits {
                wavelength: (0.0, 0.0),
                power: (0.0, 0.0),
            },
        }
    }

    pub fn from_values(values: Vec<SpectrumValue>) -> Spectrum {
        if values.len() == 0 {
            return Self::empty();
        }

        // Can unwrap, size checked
        let first_wl = values.first().unwrap().wavelength;
        let last_wl = values.last().unwrap().wavelength;
        let limits_wl = [first_wl, last_wl]
            .iter()
            .fold((f64::INFINITY, f64::NEG_INFINITY), |acc, &new| {
                (acc.0.min(new), acc.1.max(new))
            });

        let limits_pwr = values
            .iter()
            .map(|value| value.power)
            .fold((f64::INFINITY, f64::NEG_INFINITY), |acc, new| {
                (acc.0.min(new), acc.1.max(new))
            });

        let limits = Limits {
            wavelength: limits_wl,
            power: limits_pwr,
        };

        Spectrum {
            values,
            valleys: None,
            limits,
        }
    }

    pub fn from_str(text: &str) -> Result<Spectrum, Box<dyn Error>> {
        let mut csv_reader = csv::ReaderBuilder::new()
            .delimiter(b';')
            .has_headers(false)
            .from_reader(text.as_bytes());

        let readings: Result<Vec<SpectrumValue>, _> = csv_reader.deserialize().collect();

        match readings {
            Ok(values) => Ok(Self::from_values(values)),
            Err(err) => Err(Box::new(err)),
        }
    }

    pub fn to_path(&self, svg_limits: (u32, u32), graph_limits: &Limits) -> String {
        let svg_limits = (svg_limits.0 as f64 - 40.0, svg_limits.1 as f64 - 16.6);

        // let limits_pwr = (graph_limits.power.1, graph_limits.power.0);        // Invert because svg coords
        // let limits_wl = graph_limits.wavelength;    // TODO remove

        if self.values.len() == 0 {
            return "".to_string();
        }

        let cvt = |point| convert_point(&graph_limits, &svg_limits, point);
        let start = cvt(&self.values[0]);
        let start = format!("M {:.2},{:.2} ", start.0, start.1);

        let last_entry = self.values.last().unwrap(); // The size is checked above

        let path = &self
            .values
            .iter()
            .skip(1)
            .chain((0..3).map(|_| last_entry)) // Without this the end is cropped
            .map(cvt)
            .tuple_windows() // Cropped because of the window
            .map(|(a, b, c, d)| bezier_point(a, b, c, d))
            .collect::<String>();
        let path = format!("{start}{path}");

        return path.to_string();
    }

    pub fn get_limits(&self) -> Limits {
        let mut wl_min: f64 = 10_000.0; // Values that will always be outside the range
        let mut wl_max: f64 = 0.0;
        let mut pwr_min: f64 = 1000.0;
        let mut pwr_max: f64 = -1000.0;

        for value in self.values.iter() {
            if value.wavelength > wl_max {
                wl_max = value.wavelength;
            }
            if value.wavelength < wl_min {
                wl_min = value.wavelength;
            }
            if value.power > pwr_max {
                pwr_max = value.power;
            }
            if value.power < pwr_min {
                pwr_min = value.power;
            }
        }

        Limits {
            wavelength: (wl_min, wl_max),
            power: (pwr_min - 3.0, pwr_max + 3.0),
        }
    }

    pub fn save(&self, path: &Path) -> Result<(), Box<dyn Error>> {
        let mut writer = csv::WriterBuilder::new()
            .has_headers(false)
            .delimiter(b';')
            .from_path(path)?;

        let result = self
            .values
            .iter()
            .map(|entry| writer.serialize(entry))
            .collect::<Result<Vec<()>, csv::Error>>();

        match result {
            Ok(_) => Ok(()),
            Err(error) => Err(Box::new(error)),
        }
    }
}

impl Spectrum {
    pub fn get_valleys(&mut self) -> &Vec<SpectrumValue> {
        let powers: Vec<f64> = self
            .values
            .iter()
            .map(|spectrum_value| -spectrum_value.power)
            .collect();

        let mut peak_finder = PeakFinder::new(&powers);
        peak_finder.with_min_prominence(1.0); // TODO to config

        let valleys: Vec<SpectrumValue> = peak_finder
            .find_peaks()
            .iter()
            .map(|peak| self.values[peak.middle_position()].clone())
            .collect();

        self.valleys = Some(valleys);
        (self.valleys.as_ref()).expect("Just put it in a Some, so should be valid")
    }

    // TODO implement different methods: None, Simple, Lorentz, Gauss
    pub fn get_valleys_points(
        &mut self,
        svg_limits: (u32, u32),
        graph_limits: &Limits,
    ) -> Vec<(f64, f64)> {
        let svg_limits = (svg_limits.0 as f64 - 40.0, svg_limits.1 as f64 - 16.6);

        let valleys = match &self.valleys {
            Some(valleys) => valleys,
            None => self.get_valleys(),
        };

        valleys
            .iter()
            .map(|valley| convert_point(&graph_limits, &svg_limits, valley))
            .collect()
    }
}
