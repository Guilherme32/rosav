#![allow(dead_code)]

use chrono::prelude::*;
use csv;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::f64::consts::PI;
use std::fmt;
use std::path::Path;

use find_peaks::PeakFinder;
use itertools::Itertools;

use nalgebra::DVector;
use std::ops::Range;
use varpro::prelude::*;
use varpro::solvers::levmar::{LevMarProblemBuilder, LevMarSolver};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SpectrumValue {
    pub wavelength: f64,
    pub power: f64,
}

#[derive(Debug, Clone)]
pub struct Spectrum {
    pub values: Vec<SpectrumValue>,
    pub limits: Limits,
    pub info: Info,
}

#[derive(Debug, Clone)]
pub struct Info {
    pub name: Option<String>,
    pub save_time: String,
    pub valleys: Option<Vec<SpectrumValue>>,
    pub valley_detection: ValleyDetection,
    pub peaks: Option<Vec<SpectrumValue>>,
}

#[derive(Debug, Clone)]
pub struct Limits {
    pub wavelength: (f64, f64),
    pub power: (f64, f64),
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(tag = "type")]
pub enum ValleyDetection {
    None,
    Simple { prominence: f64 },
    Lorentz { prominence: f64 },
}

impl Info {
    pub fn from_now() -> Info {
        let save_time = Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

        Info {
            name: None,
            save_time,
            valleys: None,
            valley_detection: ValleyDetection::None,
            peaks: None,
        }
    }
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
        let info = Info::from_now();

        Spectrum {
            values: vec![],
            info,
            limits: Limits {
                wavelength: (3000.0, -3000.0),
                power: (100.0, -100.0),
            },
        }
    }

    pub fn from_values(values: Vec<SpectrumValue>) -> Spectrum {
        if values.is_empty() {
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

        let info = Info::from_now();
        Spectrum {
            values,
            info,
            limits,
        }
    }

    pub fn from_csv_text(text: &str) -> Result<Spectrum, Box<dyn Error>> {
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

        if self.values.is_empty() {
            return "".to_string();
        }

        let cvt = |point| convert_point(graph_limits, &svg_limits, point);
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

        path
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
    pub fn get_valleys(&mut self, method: ValleyDetection) -> Option<&Vec<SpectrumValue>> {
        match method {
            ValleyDetection::None => None,
            ValleyDetection::Simple { prominence } => Some(self.get_valleys_simple(prominence)),
            ValleyDetection::Lorentz { prominence } => Some(self.get_valleys_lorentz(prominence)),
        }
    }

    pub fn get_valleys_simple(&mut self, prominence: f64) -> &Vec<SpectrumValue> {
        let powers: Vec<f64> = self
            .values
            .iter()
            .map(|spectrum_value| -spectrum_value.power)
            .collect();

        let mut peak_finder = PeakFinder::new(&powers);
        peak_finder.with_min_prominence(prominence);

        let valleys: Vec<SpectrumValue> = peak_finder
            .find_peaks()
            .iter()
            .map(|peak| self.values[peak.middle_position()].clone())
            .collect();

        self.info.valleys = Some(valleys);
        self.info.valley_detection = ValleyDetection::Simple { prominence };

        // Can unwrap, just put it in a Some
        (self.info.valleys.as_ref()).unwrap()
    }

    pub fn get_valley_range(&self, peak: &find_peaks::Peak<f64>) -> Range<usize> {
        let mut left = peak.middle_position();
        let mut right = left;

        let valley_pwr = self.values[peak.middle_position()].power;
        let prominence = peak.prominence.unwrap_or(3.0);

        for i in (0..left).rev() {
            if self.values[i].power >= valley_pwr + prominence / 2.0 {
                left = i;
                break;
            }
        }
        for i in right..self.values.len() {
            if self.values[i].power >= valley_pwr + prominence / 2.0 {
                right = i;
                break;
            }
        }

        left..right
    }

    pub fn get_valleys_lorentz(&mut self, prominence: f64) -> &Vec<SpectrumValue> {
        let powers: Vec<f64> = self
            .values
            .iter()
            .map(|spectrum_value| -spectrum_value.power)
            .collect();

        let mut peak_finder = PeakFinder::new(&powers);
        peak_finder.with_min_prominence(prominence);

        let valleys: Option<Vec<SpectrumValue>> = peak_finder
            .find_peaks()
            .iter()
            .map(|peak| self.get_valley_range(peak))
            .map(|peak| approximate_lorentz(&self.values[peak]))
            .filter(|valley| valley.is_some())
            .collect();

        self.info.valleys = valleys;
        self.info.valley_detection = ValleyDetection::Lorentz { prominence };
        if self.info.valleys.is_none() {
            self.info.valleys = Some(vec![]);
        }

        // Can unwrap, just put it in a Some
        (self.info.valleys.as_ref()).unwrap()
    }

    pub fn get_valleys_points(
        &mut self,
        svg_limits: (u32, u32),
        graph_limits: &Limits,
        method: ValleyDetection,
    ) -> Vec<(f64, f64)> {
        let svg_limits = (svg_limits.0 as f64 - 40.0, svg_limits.1 as f64 - 16.6);

        let valleys_option = if method == self.info.valley_detection {
            self.info.valleys.as_ref()
        } else {
            self.get_valleys(method)
        };

        valleys_option
            .map(|valleys| {
                valleys
                    .iter()
                    .map(|valley| convert_point(graph_limits, &svg_limits, valley))
                    .collect()
            })
            .unwrap_or(vec![])
    }
}

pub fn lorentz(x: &DVector<f64>, x_0: f64, gamma: f64) -> DVector<f64> {
    x.map(|x| (1.0 / (PI * gamma * (1.0 + ((x - x_0) / gamma).powf(2.0)))))
}

pub fn derivative_lorentz_x_0(x: &DVector<f64>, x_0: f64, gamma: f64) -> DVector<f64> {
    x.map(|x| {
        (2.0 / (PI * gamma.powf(3.0))) * (x - x_0) / (1.0 + ((x - x_0) / gamma).powf(2.0)).powf(2.0)
    })
}

pub fn derivative_lorentz_gamma(x: &DVector<f64>, x_0: f64, gamma: f64) -> DVector<f64> {
    x.map(|x| {
        let a = ((x - x_0) / gamma).powf(2.0);
        (-1.0 / (PI * gamma.powf(2.0))) * ((1.0 - a) / (1.0 + a).powf(2.0))
    })
}

pub fn approximate_lorentz(values: &[SpectrumValue]) -> Option<SpectrumValue> {
    let x: Vec<f64> = values.iter().map(|value| value.wavelength).collect();
    let x = DVector::from(x);
    let y: Vec<f64> = values.iter().map(|value| value.power).collect();
    let y = DVector::from(y);

    let guess_x_0 = (x[x.len() - 1] + x[0]) / 2.0; // Rough center
    let guess_gamma = (x[x.len() - 1] - x[0]) / 2.0; // Rough 1/2 FWHM (FWHM = 2 gamma)

    let initial_guess = vec![guess_x_0, guess_gamma];

    // NOTE these unwraps for the builders should work, but keep an eye on them
    let model = SeparableModelBuilder::<f64>::new(&["x_0", "gamma"])
        .function(&["x_0", "gamma"], lorentz)
        .partial_deriv("x_0", derivative_lorentz_x_0)
        .partial_deriv("gamma", derivative_lorentz_gamma)
        .invariant_function(|x| DVector::from_element(x.len(), 1.))
        .independent_variable(x)
        .initial_parameters(initial_guess)
        .build()
        .unwrap();

    let problem = LevMarProblemBuilder::new(model)
        .observations(y)
        .build()
        .unwrap();

    let (solved_problem, report) = LevMarSolver::new().with_xtol(1e-12).minimize(problem);

    if !report.termination.was_successful() {
        println!(
            "Lorentz approximation termination unsuccessfull: {:?}",
            report
        );
        return None;
    }

    let optimized_params = solved_problem.params();
    let coeffs = solved_problem.linear_coefficients().unwrap();

    let wavelength = vec![optimized_params[0]];
    let wavelength = DVector::from(wavelength);

    let power =
        lorentz(&wavelength, optimized_params[0], optimized_params[1])[0] * coeffs[0] + coeffs[1];

    let min_wl = values[0].wavelength;
    let max_wl = values[values.len() - 1].wavelength;
    if !(min_wl..=max_wl).contains(&wavelength[0]) {
        // Wavelength calculated outside the valley
        return None;
    }

    Some(SpectrumValue {
        wavelength: wavelength[0],
        power,
    })
}
