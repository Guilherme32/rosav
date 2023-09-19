#![allow(dead_code)]

use chrono::prelude::*;
use csv;
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::f64::consts::PI;
use std::fmt;
use std::path::Path;

use find_peaks::PeakFinder;
use nalgebra::DVector;
use std::ops::Range;
use varpro::prelude::*;
use varpro::solvers::levmar::{LevMarProblemBuilder, LevMarSolver};

use crate::svg_utils::*;

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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Info {
    pub name: Option<String>,
    pub save_time: String,
    pub valleys: Option<Vec<SpectrumValue>>,
    pub valley_detection: CriticalDetection,
    pub peaks: Option<Vec<SpectrumValue>>,
    pub peak_detection: CriticalDetection,
}

#[derive(Debug, Clone)]
pub struct Limits {
    pub wavelength: (f64, f64),
    pub power: (f64, f64),
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
#[serde(tag = "type")]
pub enum CriticalDetection {
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
            valley_detection: CriticalDetection::None,
            peaks: None,
            peak_detection: CriticalDetection::None,
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
        let graph_limits = GraphLimits {
            x: graph_limits.wavelength,
            y: graph_limits.power,
        };
        let points: Vec<(f64, f64)> = self
            .values
            .iter()
            .map(|value| (value.wavelength, value.power))
            .collect();

        bezier_path(&points, svg_limits, &graph_limits)
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
    pub fn find_valleys(&mut self, method: CriticalDetection) -> Option<&Vec<SpectrumValue>> {
        match method {
            CriticalDetection::None => None,
            CriticalDetection::Simple { prominence } => {
                Some(self.get_peaks_simple(prominence, true))
            }
            CriticalDetection::Lorentz { prominence } => {
                Some(self.get_peaks_lorentz(prominence, true))
            }
        }
    }

    pub fn find_peaks(&mut self, method: CriticalDetection) -> Option<&Vec<SpectrumValue>> {
        match method {
            CriticalDetection::None => None,
            CriticalDetection::Simple { prominence } => {
                Some(self.get_peaks_simple(prominence, false))
            }
            CriticalDetection::Lorentz { prominence } => {
                Some(self.get_peaks_lorentz(prominence, false))
            }
        }
    }

    pub fn get_peaks_simple(&mut self, prominence: f64, invert: bool) -> &Vec<SpectrumValue> {
        let signal = if invert { -1.0 } else { 1.0 };
        let powers: Vec<f64> = self
            .values
            .iter()
            .map(|spectrum_value| signal * spectrum_value.power)
            .collect();

        let mut peak_finder = PeakFinder::new(&powers);
        peak_finder.with_min_prominence(prominence);

        let peaks: Vec<SpectrumValue> = peak_finder
            .find_peaks()
            .iter()
            .map(|peak| self.values[peak.middle_position()].clone())
            .collect();

        if invert {
            self.info.valleys = Some(peaks);
            self.info.valley_detection = CriticalDetection::Simple { prominence };

            // Can unwrap, just put it in a Some
            (self.info.valleys.as_ref()).unwrap()
        } else {
            self.info.peaks = Some(peaks);
            self.info.peak_detection = CriticalDetection::Simple { prominence };

            // Can unwrap, just put it in a Some
            (self.info.peaks.as_ref()).unwrap()
        }
    }

    pub fn get_peak_range(&self, peak: &find_peaks::Peak<f64>, invert: bool) -> Range<usize> {
        let mut left = peak.middle_position();
        let mut right = left;

        let signal = if invert { -1.0 } else { 1.0 };

        let peak_pwr = signal * self.values[peak.middle_position()].power;
        let prominence = peak.prominence.unwrap_or(3.0);

        for i in (0..left).rev() {
            if signal * self.values[i].power <= peak_pwr - prominence / 2.0 {
                left = i;
                break;
            }
        }
        for i in right..self.values.len() {
            if signal * self.values[i].power <= peak_pwr - prominence / 2.0 {
                right = i;
                break;
            }
        }

        left..right
    }

    pub fn get_peaks_lorentz(&mut self, prominence: f64, invert: bool) -> &Vec<SpectrumValue> {
        let signal = if invert { -1.0 } else { 1.0 };
        let powers: Vec<f64> = self
            .values
            .iter()
            .map(|spectrum_value| signal * spectrum_value.power)
            .collect();

        let mut peak_finder = PeakFinder::new(&powers);
        peak_finder.with_min_prominence(prominence);

        let peaks: Option<Vec<SpectrumValue>> = peak_finder
            .find_peaks()
            .iter()
            .map(|peak| self.get_peak_range(peak, invert))
            .map(|peak| approximate_lorentz(&self.values[peak]))
            .filter(|valley| valley.is_some())
            .collect();

        if invert {
            self.info.valleys = peaks;
            self.info.valley_detection = CriticalDetection::Lorentz { prominence };
            if self.info.valleys.is_none() {
                self.info.valleys = Some(vec![]);
            }

            // Can unwrap, just put it in a Some
            (self.info.valleys.as_ref()).unwrap()
        } else {
            self.info.peaks = peaks;
            self.info.peak_detection = CriticalDetection::Lorentz { prominence };
            if self.info.peaks.is_none() {
                self.info.peaks = Some(vec![]);
            }

            // Can unwrap, just put it in a Some
            (self.info.peaks.as_ref()).unwrap()
        }
    }

    pub fn get_valleys(&mut self, method: CriticalDetection) -> Vec<SpectrumValue> {
        let valleys_option = if method == self.info.valley_detection {
            self.info.valleys.as_ref()
        } else {
            self.find_valleys(method)
        };

        if let Some(valleys) = valleys_option {
            valleys.clone()
        } else {
            vec![]
        }
    }

    pub fn get_valleys_points(
        &mut self,
        svg_limits: (u32, u32),
        graph_limits: &Limits,
        method: CriticalDetection,
    ) -> Vec<(f64, f64)> {
        let svg_limits = (svg_limits.0 as f64 - 40.0, svg_limits.1 as f64 - 16.6);
        let graph_limits = GraphLimits {
            x: graph_limits.wavelength,
            y: graph_limits.power,
        };

        let valleys = self.get_valleys(method);
        valleys
            .iter()
            .map(|valley| (valley.wavelength, valley.power))
            .map(|valley| convert_point(&graph_limits, &svg_limits, &valley))
            .collect()
    }

    pub fn get_peaks(&mut self, method: CriticalDetection) -> Vec<SpectrumValue> {
        let peaks_option = if method == self.info.peak_detection {
            self.info.peaks.as_ref()
        } else {
            self.find_peaks(method)
        };

        if let Some(peaks) = peaks_option {
            peaks.clone()
        } else {
            vec![]
        }
    }

    pub fn get_peaks_points(
        &mut self,
        svg_limits: (u32, u32),
        graph_limits: &Limits,
        method: CriticalDetection,
    ) -> Vec<(f64, f64)> {
        let svg_limits = (svg_limits.0 as f64 - 40.0, svg_limits.1 as f64 - 16.6);
        let graph_limits = GraphLimits {
            x: graph_limits.wavelength,
            y: graph_limits.power,
        };

        let peaks = self.get_peaks(method);
        peaks
            .iter()
            .map(|peak| (peak.wavelength, peak.power))
            .map(|peak| convert_point(&graph_limits, &svg_limits, &peak))
            .collect()
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

    if x.is_empty() {
        return None;
    }

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
