//! Simple line-based parser for airspace files in `OpenAir` format (used by
//! flight instruments like Skytraxx and others).
//!
//! <http://www.winpilot.com/UsersGuide/UserAirspace.asp>
//!
//! If you want to use this library, you need the [`parse`](fn.parse.html)
//! function as entry point.
//!
//! For an example on how to use the parse function, see the examples in the
//! source repository.
//!
//! ## Implementation Notes
//!
//! Unfortunately the `OpenAir` format is really underspecified. Every device
//! uses varying conventions. For example, there is nothing we can use as clear
//! delimiter for airspaces. Some files delimit airspaces with an empty line,
//! some with a comment. But on the other hand, some files even place comments
//! between the coordinates so that they cannot be used as delimiter either.
//!
//! This parser tries to be very lenient when parsing, based on real life data.
//! The end of an airspace is reached when the next one starts (with an `AC`
//! record) or when the file ends.
//!
//! Note: AT records (label placement hints) are currently ignored
#![deny(clippy::all)]

mod activations;
mod altitude;
mod classes;
mod coords;
mod geometry;
mod record;

use std::{fmt, io::BufRead, mem};

use log::{debug, trace};
#[cfg(feature = "serde")]
use serde::Serialize;

use crate::record::Record;
pub use crate::{
    activations::ActivationTimes,
    altitude::Altitude,
    classes::Class,
    coords::Coord,
    geometry::{Arc, ArcSegment, Direction, Geometry, PolygonSegment},
};

/// An airspace.
#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct Airspace {
    /// The name / description of the airspace
    pub name: String,
    /// The airspace class
    pub class: Class,
    /// The airspace type (extension record)
    #[cfg_attr(feature = "serde", serde(rename = "type"))]
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub type_: Option<String>,
    /// The lower bound of the airspace
    pub lower_bound: Altitude,
    /// The upper bound of the airspace
    pub upper_bound: Altitude,
    /// The airspace geometry
    pub geom: Geometry,
    /// Frequency of the controlling ATC-station or other authority in that
    /// particular airspace (extension record)
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub frequency: Option<String>,
    /// Call-sign for this station
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub call_sign: Option<String>,
    /// Transponder code associated with this airspace
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub transponder_code: Option<u16>,
    /// Airspace activation times
    #[cfg_attr(feature = "serde", serde(skip_serializing_if = "Option::is_none"))]
    pub activation_times: Option<ActivationTimes>,
}

impl fmt::Display for Airspace {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "{} [{}] ({} → {}) {{{}}}",
            self.name, self.class, self.lower_bound, self.upper_bound, self.geom,
        )
    }
}

/// An incomplete airspace.
#[derive(Debug)]
struct AirspaceBuilder {
    // Base records
    new: bool,
    name: Option<String>,
    class: Option<Class>,
    lower_bound: Option<Altitude>,
    upper_bound: Option<Altitude>,
    geom: Option<Geometry>,

    // Extension records
    type_: Option<String>,
    frequency: Option<String>,
    call_sign: Option<String>,
    transponder_code: Option<u16>,
    activation_times: Option<ActivationTimes>,

    // Variables
    var_x: Option<Coord>,
    var_d: Option<Direction>,
}

macro_rules! setter {
    (ONCE, $method:ident, $field:ident, $type:ty) => {
        fn $method(&mut self, $field: $type) -> Result<(), String> {
            self.new = false;
            if self.$field.is_some() {
                Err(format!(
                    "Could not set {} (already defined)",
                    stringify!($field)
                ))
            } else {
                self.$field = Some($field);
                Ok(())
            }
        }
    };
    (MANY, $method:ident, $field:ident, $type:ty) => {
        fn $method(&mut self, $field: $type) {
            self.new = false;
            self.$field = Some($field);
        }
    };
}

impl AirspaceBuilder {
    fn new() -> Self {
        Self {
            new: true,
            name: None,
            class: None,
            lower_bound: None,
            upper_bound: None,
            geom: None,
            type_: None,
            frequency: None,
            call_sign: None,
            transponder_code: None,
            activation_times: None,
            var_x: None,
            var_d: None,
        }
    }

    setter!(ONCE, set_name, name, String);
    setter!(ONCE, set_class, class, Class);
    setter!(ONCE, set_lower_bound, lower_bound, Altitude);
    setter!(ONCE, set_upper_bound, upper_bound, Altitude);
    setter!(ONCE, set_type, type_, String);
    setter!(ONCE, set_frequency, frequency, String);
    setter!(ONCE, set_call_sign, call_sign, String);
    setter!(ONCE, set_transponder_code, transponder_code, u16);
    setter!(
        ONCE,
        set_activation_times,
        activation_times,
        ActivationTimes
    );
    setter!(MANY, set_var_x, var_x, Coord);
    setter!(MANY, set_var_d, var_d, Direction);

    fn add_segment(&mut self, segment: PolygonSegment) -> Result<(), String> {
        self.new = false;
        match &mut self.geom {
            None => {
                self.geom = Some(Geometry::Polygon {
                    segments: vec![segment],
                })
            }
            Some(Geometry::Polygon { segments }) => {
                segments.push(segment);
            }
            Some(Geometry::Circle { .. }) => {
                return Err("Cannot add a point to a circle".into());
            }
        }
        Ok(())
    }

    fn set_circle_radius(&mut self, radius: f32) -> Result<(), String> {
        self.new = false;
        match (&self.geom, &self.var_x) {
            (None, Some(centerpoint)) => {
                self.geom = Some(Geometry::Circle {
                    centerpoint: centerpoint.clone(),
                    radius,
                });
                Ok(())
            }
            (Some(_), _) => Err("Geometry already set".into()),
            (_, None) => Err("Centerpoint missing".into()),
        }
    }

    fn finish(self) -> Result<Airspace, String> {
        debug!("Finish {:?}", self.name);
        let name = self.name.ok_or("Missing name")?;
        let class = self
            .class
            .ok_or_else(|| format!("Missing class for '{name}'"))?;
        let lower_bound = self
            .lower_bound
            .ok_or_else(|| format!("Missing lower bound for '{name}'"))?;
        let upper_bound = self
            .upper_bound
            .ok_or_else(|| format!("Missing upper bound for '{name}'"))?;
        let geom = self
            .geom
            .ok_or_else(|| format!("Missing geom for '{name}'"))?;
        Ok(Airspace {
            name,
            class,
            type_: self.type_,
            lower_bound,
            upper_bound,
            geom,
            frequency: self.frequency,
            call_sign: self.call_sign,
            transponder_code: self.transponder_code,
            activation_times: self.activation_times,
        })
    }
}

/// Process a record.
fn process(builder: &mut AirspaceBuilder, record: Record) -> Result<(), String> {
    match record {
        Record::Empty => {}
        Record::Comment => {}
        Record::LabelPlacement => {}
        Record::Pen => {}
        Record::Brush => {}
        Record::UnknownExtension(_) => {}
        Record::AirspaceClass(class) => {
            builder.set_class(class)?;
        }
        Record::AirspaceName(name) => {
            builder.set_name(name.to_string())?;
        }
        Record::LowerBound(altitude) => {
            builder.set_lower_bound(altitude)?;
        }
        Record::UpperBound(altitude) => {
            builder.set_upper_bound(altitude)?;
        }
        Record::AirspaceType(type_) => {
            builder.set_type(type_.to_string())?;
        }
        Record::Frequency(frequency) => {
            builder.set_frequency(frequency.to_string())?;
        }
        Record::CallSign(call_sign) => {
            builder.set_call_sign(call_sign.to_string())?;
        }
        Record::TransponderCode(code) => {
            builder.set_transponder_code(code)?;
        }
        Record::ActivationTimes(activation_times) => {
            builder.set_activation_times(activation_times)?;
        }
        Record::VarX(coord) => {
            builder.set_var_x(coord);
        }
        Record::VarD(direction) => {
            builder.set_var_d(direction);
        }
        Record::Point(coord) => {
            builder.add_segment(PolygonSegment::Point(coord))?;
        }
        Record::CircleRadius(radius) => {
            builder.set_circle_radius(radius)?;
        }
        Record::ArcSegmentData {
            radius,
            angle_start,
            angle_end,
        } => {
            let centerpoint = builder.var_x.clone().ok_or("Centerpoint missing")?;
            let direction = builder.var_d.unwrap_or_default();
            let arc_segment = ArcSegment {
                centerpoint,
                radius,
                angle_start,
                angle_end,
                direction,
            };
            builder.add_segment(PolygonSegment::ArcSegment(arc_segment))?;
        }
        Record::ArcData { start, end } => {
            let centerpoint = builder.var_x.clone().ok_or("Centerpoint missing")?;
            let direction = builder.var_d.unwrap_or_default();
            let arc = Arc {
                centerpoint,
                start,
                end,
                direction,
            };
            builder.add_segment(PolygonSegment::Arc(arc))?;
        }
    }
    Ok(())
}

/// Process the reader until EOF, return a list of found airspaces.
pub fn parse<R: BufRead>(reader: &mut R) -> Result<Vec<Airspace>, String> {
    let mut airspaces = vec![];

    let mut builder = AirspaceBuilder::new();
    let mut buf: Vec<u8> = vec![];
    loop {
        // Read next line
        buf.clear();
        let bytes_read = reader
            .read_until(0x0a /*\n*/, &mut buf)
            .map_err(|e| format!("Could not read line: {e}"))?;
        if bytes_read == 0 {
            // EOF
            trace!("Reached EOF");
            airspaces.push(builder.finish()?);
            return Ok(airspaces);
        }
        let line = String::from_utf8_lossy(&buf);

        // Trim BOM
        let trimmed_line = line.trim_start_matches('\u{feff}');

        // Parse the record
        let record = Record::parse(trimmed_line)?;

        // Determine whether we reached the start of a new airspace
        let start_of_airspace = matches!(record, Record::AirspaceClass(_));

        // A new airspace starts, collect the old one first
        if start_of_airspace && !builder.new {
            let old_builder = mem::replace(&mut builder, AirspaceBuilder::new());
            airspaces.push(old_builder.finish()?);
        }

        // Process current record
        process(&mut builder, record)?;
    }
}
