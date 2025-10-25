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

use std::{
    fmt,
    io::{BufRead, Write},
    mem,
};

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

impl Airspace {
    /// Writes the airspace in OpenAir format.
    pub fn write<W: Write>(&self, mut writer: W) -> std::io::Result<()> {
        // 1. AC (class) - required
        Record::AirspaceClass(self.class).write(&mut writer)?;

        // 2. AY (type) - optional
        if let Some(ref type_) = self.type_ {
            Record::AirspaceType(type_).write(&mut writer)?;
        }

        // 3. AN (name) - required
        Record::AirspaceName(&self.name).write(&mut writer)?;

        // 4. AL (lower bound) - required
        Record::LowerBound(self.lower_bound.clone()).write(&mut writer)?;

        // 5. AH (upper bound) - required
        Record::UpperBound(self.upper_bound.clone()).write(&mut writer)?;

        // 6. AF (frequency) - optional
        if let Some(ref frequency) = self.frequency {
            Record::Frequency(frequency).write(&mut writer)?;
        }

        // 7. AG (call sign) - optional
        if let Some(ref call_sign) = self.call_sign {
            Record::CallSign(call_sign).write(&mut writer)?;
        }

        // 8. AX (transponder code) - optional
        if let Some(transponder_code) = self.transponder_code {
            Record::TransponderCode(transponder_code).write(&mut writer)?;
        }

        // 9. AA (activation times) - optional
        if let Some(activation_times) = self.activation_times {
            Record::ActivationTimes(activation_times).write(&mut writer)?;
        }

        // 10. Geometry
        match &self.geom {
            Geometry::Circle {
                centerpoint,
                radius,
            } => {
                Record::VarX(centerpoint.clone()).write(&mut writer)?;
                Record::CircleRadius(*radius).write(&mut writer)?;
            }
            Geometry::Polygon { segments } => {
                for segment in segments {
                    match segment {
                        PolygonSegment::Point(coord) => {
                            Record::Point(coord.clone()).write(&mut writer)?;
                        }
                        PolygonSegment::ArcSegment(arc_segment) => {
                            Record::VarX(arc_segment.centerpoint.clone()).write(&mut writer)?;
                            Record::VarD(arc_segment.direction).write(&mut writer)?;
                            Record::ArcSegmentData {
                                radius: arc_segment.radius,
                                angle_start: arc_segment.angle_start,
                                angle_end: arc_segment.angle_end,
                            }
                            .write(&mut writer)?;
                        }
                        PolygonSegment::Arc(arc) => {
                            Record::VarX(arc.centerpoint.clone()).write(&mut writer)?;
                            Record::VarD(arc.direction).write(&mut writer)?;
                            Record::ArcData {
                                start: arc.start.clone(),
                                end: arc.end.clone(),
                            }
                            .write(&mut writer)?;
                        }
                    }
                }
            }
        }

        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    fn write_airspace(airspace: &Airspace) -> String {
        let mut buf = Vec::new();
        airspace.write(&mut buf).unwrap();
        String::from_utf8(buf).unwrap()
    }

    #[test]
    fn write_minimal_circle() {
        let airspace = Airspace {
            name: "Test Zone".to_string(),
            class: Class::D,
            type_: None,
            lower_bound: Altitude::Gnd,
            upper_bound: Altitude::FlightLevel(100),
            geom: Geometry::Circle {
                centerpoint: Coord {
                    lat: 47.0,
                    lng: 8.0,
                },
                radius: 5.0,
            },
            frequency: None,
            call_sign: None,
            transponder_code: None,
            activation_times: None,
        };

        insta::assert_snapshot!(write_airspace(&airspace), @r"
        AC D
        AN Test Zone
        AL GND
        AH FL100
        V X=47:00:00 N 008:00:00 E
        DC 5
        ");
    }

    #[test]
    fn write_full_circle() {
        let airspace = Airspace {
            name: "Full Test Zone".to_string(),
            class: Class::Ctr,
            type_: Some("CTR".to_string()),
            lower_bound: Altitude::FeetAmsl(1000),
            upper_bound: Altitude::FeetAmsl(5000),
            geom: Geometry::Circle {
                centerpoint: Coord {
                    lat: 46.5,
                    lng: 9.5,
                },
                radius: 10.0,
            },
            frequency: Some("123.45".to_string()),
            call_sign: Some("TOWER".to_string()),
            transponder_code: Some(7000),
            activation_times: Some("2023-12-16T12:00Z/2023-12-16T13:00Z".parse().unwrap()),
        };

        insta::assert_snapshot!(write_airspace(&airspace), @r"
        AC CTR
        AY CTR
        AN Full Test Zone
        AL 1000ft AMSL
        AH 5000ft AMSL
        AF 123.45
        AG TOWER
        AX 7000
        AA 2023-12-16T12:00:00.0+00:00/2023-12-16T13:00:00.0+00:00
        V X=46:30:00 N 009:30:00 E
        DC 10
        ");
    }

    #[test]
    fn write_polygon_with_points() {
        let airspace = Airspace {
            name: "Polygon Zone".to_string(),
            class: Class::A,
            type_: None,
            lower_bound: Altitude::Gnd,
            upper_bound: Altitude::Unlimited,
            geom: Geometry::Polygon {
                segments: vec![
                    PolygonSegment::Point(Coord {
                        lat: 47.0,
                        lng: 8.0,
                    }),
                    PolygonSegment::Point(Coord {
                        lat: 47.0,
                        lng: 9.0,
                    }),
                    PolygonSegment::Point(Coord {
                        lat: 46.0,
                        lng: 9.0,
                    }),
                ],
            },
            frequency: None,
            call_sign: None,
            transponder_code: None,
            activation_times: None,
        };

        insta::assert_snapshot!(write_airspace(&airspace), @r"
        AC A
        AN Polygon Zone
        AL GND
        AH UNLIM
        DP 47:00:00 N 008:00:00 E
        DP 47:00:00 N 009:00:00 E
        DP 46:00:00 N 009:00:00 E
        ");
    }

    #[test]
    fn write_polygon_with_arc_segment() {
        let airspace = Airspace {
            name: "Arc Segment Zone".to_string(),
            class: Class::Restricted,
            type_: None,
            lower_bound: Altitude::FeetAgl(0),
            upper_bound: Altitude::FeetAmsl(3000),
            geom: Geometry::Polygon {
                segments: vec![
                    PolygonSegment::Point(Coord {
                        lat: 47.0,
                        lng: 8.0,
                    }),
                    PolygonSegment::ArcSegment(ArcSegment {
                        centerpoint: Coord {
                            lat: 47.0,
                            lng: 8.5,
                        },
                        radius: 10.0,
                        angle_start: 270.0,
                        angle_end: 290.0,
                        direction: Direction::Cw,
                    }),
                ],
            },
            frequency: None,
            call_sign: None,
            transponder_code: None,
            activation_times: None,
        };

        insta::assert_snapshot!(write_airspace(&airspace), @r"
        AC R
        AN Arc Segment Zone
        AL 0ft AGL
        AH 3000ft AMSL
        DP 47:00:00 N 008:00:00 E
        V X=47:00:00 N 008:30:00 E
        V D=+
        DA 10, 270, 290
        ");
    }

    #[test]
    fn write_polygon_with_arc() {
        let airspace = Airspace {
            name: "Arc Zone".to_string(),
            class: Class::Danger,
            type_: None,
            lower_bound: Altitude::Gnd,
            upper_bound: Altitude::FlightLevel(50),
            geom: Geometry::Polygon {
                segments: vec![PolygonSegment::Arc(Arc {
                    centerpoint: Coord {
                        lat: 47.0,
                        lng: 8.0,
                    },
                    start: Coord {
                        lat: 47.0,
                        lng: 8.5,
                    },
                    end: Coord {
                        lat: 47.5,
                        lng: 8.0,
                    },
                    direction: Direction::Ccw,
                })],
            },
            frequency: None,
            call_sign: None,
            transponder_code: None,
            activation_times: None,
        };

        insta::assert_snapshot!(write_airspace(&airspace), @r"
        AC Q
        AN Arc Zone
        AL GND
        AH FL50
        V X=47:00:00 N 008:00:00 E
        V D=-
        DB 47:00:00 N 008:30:00 E, 47:30:00 N 008:00:00 E
        ");
    }
}
