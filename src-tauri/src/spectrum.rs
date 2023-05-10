#![allow(dead_code)]

use std::error::Error;
use std::fmt;
use csv;
use serde::Deserialize;

use itertools::Itertools;

#[derive(Debug, Deserialize)]
struct SpectrumValue {
    wavelength: f64,
    power: f64
}

#[derive(Debug)]
pub struct Spectrum {
    values: Vec<SpectrumValue>
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

    pub fn to_path(&self, svg_limits: (f64, f64)) -> String {
        let limits_pwr = (2f64, -50f64);        // #TODO to config

        if self.values.len() == 0 {
            return "".to_string();
        }

                                        // #TODO to opt config
        let limits_wl = (self.values.first().unwrap().wavelength,        // Can unwrap because length
                     self.values.last().unwrap().wavelength);             // is checked above

        let cvt = |point| convert_point(&limits_wl, &limits_pwr, &svg_limits, point);
        let start = cvt(&self.values[0]);
        let start = format!("M {:.2},{:.2} ", start.0, start.1);

        let path = &self.values.iter()
            .skip(1)
            .step_by(self.values.len()/200)            // #TODO to config (200)
            .map(cvt)
            .tuple_windows()
            .map(|(a,b,c,d)| bezier_point(a, b, c, d))
            .collect::<String>();
        let path = format!("{start}{path}");

        return path.to_string();
    }
}

