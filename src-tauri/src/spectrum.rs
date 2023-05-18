#![allow(dead_code)]

use std::error::Error;
use std::fmt;
use csv;
use serde::Deserialize;

use itertools::Itertools;

#[derive(Debug, Deserialize, Clone)]
struct SpectrumValue {
    wavelength: f64,
    power: f64
}

#[derive(Debug, Clone)]
pub struct Spectrum {
    values: Vec<SpectrumValue>
}

#[derive(Debug)]
pub struct Limits {
    pub wavelength: (f64, f64),
    pub power: (f64, f64)
}

impl fmt::Display for Spectrum {
    fn fmt(&self, f:&mut fmt::Formatter<'_>) -> fmt::Result {
        for value in &self.values {
            writeln!(f, "({:.4e}, {:.4e})", value.wavelength, value.power)?;
        }
        Ok(())
    }
}

fn convert_point(
    limits_wl: &(f64, f64),
    limits_pwr: &(f64, f64),
    svg_limits: &(f64, f64),
    og_point: &SpectrumValue
) -> (f64, f64)
{
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
    next: (f64, f64)
) -> String
{
    let smoothing = 0.1;            // #TODO to config

    let start_vector = (end.0 - previous.0, end.1 - previous.1);
    let start_control = (start.0 + start_vector.0 * smoothing,
                         start.1 + start_vector.1 * smoothing);

    let end_vector = (start.0 - next.0, start.1 - next.1);
    let end_control = (end.0 + end_vector.0 * smoothing,
                       end.1 + end_vector.1 * smoothing);

    format!("C {:.2},{:.2} {:.2},{:.2}, {:.2},{:.2} ",
        start_control.0, start_control.1,
        end_control.0, end_control.1,
        end.0, end.1)
}

impl Spectrum {
    pub fn from_str(text: &str) -> Result<Spectrum, Box<dyn Error>> {
        let mut csv_reader = csv::ReaderBuilder::new()
            .delimiter(b';')
            .has_headers(false)
            .from_reader(text.as_bytes());

        let readings: Result<Vec<SpectrumValue>, _> = csv_reader
            .deserialize()
            .collect();
        
        match readings {
            Ok(values) => Ok(Spectrum{ values }),
            Err(err) => Err(Box::new(err))
        }
    }

    pub fn to_path(&self, svg_limits: (u32, u32), graph_limits: &Limits) -> String {
        let svg_limits = (svg_limits.0 as f64 - 40.0,
                          svg_limits.1 as f64 - 16.6);

        let limits_pwr = (graph_limits.power.1, graph_limits.power.0);        // TODO to config
        let limits_wl = graph_limits.wavelength;    // TODO opt config

        if self.values.len() == 0 {
            return "".to_string();
        }

        let cvt = |point| convert_point(&limits_wl, &limits_pwr, &svg_limits, point);
        let start = cvt(&self.values[0]);
        let start = format!("M {:.2},{:.2} ", start.0, start.1);

        let path = &self.values.iter()
            .skip(1)
            .map(cvt)
            .tuple_windows()
            .map(|(a,b,c,d)| bezier_point(a, b, c, d))
            .collect::<String>();
        let path = format!("{start}{path}");

        return path.to_string();
    }

    pub fn get_limits(&self) -> Limits {
        let mut wl_min: f64 = 10_000.0;        // Values that will always be outside the range
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
            power: (pwr_min - 3.0, pwr_max + 3.0)
        }
    }
}

