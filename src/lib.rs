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
};

use log::debug;
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

struct OpenAirIterator<R: BufRead> {
    reader: R,
    line: Vec<u8>,
    use_buffered_line: bool,
}

impl<R: BufRead> OpenAirIterator<R> {
    fn new(mut reader: R) -> Self {
        let mut line = Vec::new();
        let result = reader.read_until(b'\n', &mut line);
        let use_buffered_line = result.is_ok();

        // Skip UTF8 byte-order-mark
        if line.starts_with(&[0xEF, 0xBB, 0xBF]) {
            line.drain(0..3);
        }

        Self {
            reader,
            line,
            use_buffered_line,
        }
    }

    fn next_airspace(&mut self) -> Result<Option<Airspace>, String> {
        // Local variables for accumulating airspace data
        let mut name: Option<String> = None;
        let mut class: Option<Class> = None;
        let mut lower_bound: Option<Altitude> = None;
        let mut upper_bound: Option<Altitude> = None;
        let mut geom: Option<Geometry> = None;
        let mut type_: Option<String> = None;
        let mut frequency: Option<String> = None;
        let mut call_sign: Option<String> = None;
        let mut transponder_code: Option<u16> = None;
        let mut activation_times: Option<ActivationTimes> = None;
        let mut var_x: Option<Coord> = None;
        let mut var_d: Option<Direction> = None;

        loop {
            let reached_eof = if self.use_buffered_line {
                // If we are supposed to use the buffered line, then we don't
                // involve the `reader` and just reset the flag instead.
                self.use_buffered_line = false;
                // We also apparently still have a line to process, so we are
                // not at the end of the file yet.
                false
            } else {
                // Otherwise, we should read a new line from the `reader`
                self.line.clear();
                let result = self.reader.read_until(b'\n', &mut self.line);
                let num_read = result.map_err(|e| format!("Could not read line: {e}"))?;
                // ... and if we haven't read any bytes, then we have reached
                // the end of the file.
                num_read == 0
            };

            // If we reached the end of the file, but there was no pending
            // airspace remaining, then we can "finish" the iterator.
            if reached_eof {
                // However, if we have accumulated an airspace, we should return it first
                if let Some(class) = class {
                    debug!("Finish {:?}", name);
                    let name = name.ok_or("Missing name")?;
                    let lower_bound =
                        lower_bound.ok_or_else(|| format!("Missing lower bound for '{name}'"))?;
                    let upper_bound =
                        upper_bound.ok_or_else(|| format!("Missing upper bound for '{name}'"))?;
                    let geom = geom.ok_or_else(|| format!("Missing geom for '{name}'"))?;
                    return Ok(Some(Airspace {
                        name,
                        class,
                        type_,
                        lower_bound,
                        upper_bound,
                        geom,
                        frequency,
                        call_sign,
                        transponder_code,
                        activation_times,
                    }));
                }
                return Ok(None);
            }

            // Parse the line as a Record
            let line_str = String::from_utf8_lossy(&self.line);
            let trimmed = line_str.trim_start_matches('\u{feff}');
            let record = Record::parse(trimmed)?;

            // If we see a new AirspaceClass record and we already have accumulated
            // an airspace, we should return the current airspace first.
            if matches!(record, Record::AirspaceClass(_))
                && let Some(class) = class
            {
                // Mark the current line as not consumed yet so that we can
                // reuse it in the `next()` iteration.
                self.use_buffered_line = true;

                // Build and return airspace from accumulated data
                debug!("Finish {:?}", name);
                let name = name.ok_or("Missing name")?;
                let lower_bound =
                    lower_bound.ok_or_else(|| format!("Missing lower bound for '{name}'"))?;
                let upper_bound =
                    upper_bound.ok_or_else(|| format!("Missing upper bound for '{name}'"))?;
                let geom = geom.ok_or_else(|| format!("Missing geom for '{name}'"))?;
                return Ok(Some(Airspace {
                    name,
                    class,
                    type_,
                    lower_bound,
                    upper_bound,
                    geom,
                    frequency,
                    call_sign,
                    transponder_code,
                    activation_times,
                }));
            }

            // Process the record
            match record {
                Record::Empty => {}
                Record::Comment => {}
                Record::LabelPlacement => {}
                Record::Pen => {}
                Record::Brush => {}
                Record::UnknownExtension(_) => {}
                Record::AirspaceClass(parsed_class) => {
                    if class.is_some() {
                        return Err("Could not set class (already defined)".to_string());
                    }
                    class = Some(parsed_class);
                }
                Record::AirspaceName(parsed_name) => {
                    if name.is_some() {
                        return Err("Could not set name (already defined)".to_string());
                    }
                    name = Some(parsed_name.to_string());
                }
                Record::LowerBound(altitude) => {
                    if lower_bound.is_some() {
                        return Err("Could not set lower_bound (already defined)".to_string());
                    }
                    lower_bound = Some(altitude);
                }
                Record::UpperBound(altitude) => {
                    if upper_bound.is_some() {
                        return Err("Could not set upper_bound (already defined)".to_string());
                    }
                    upper_bound = Some(altitude);
                }
                Record::AirspaceType(parsed_type) => {
                    if type_.is_some() {
                        return Err("Could not set type (already defined)".to_string());
                    }
                    type_ = Some(parsed_type.to_string());
                }
                Record::Frequency(parsed_freq) => {
                    if frequency.is_some() {
                        return Err("Could not set frequency (already defined)".to_string());
                    }
                    frequency = Some(parsed_freq.to_string());
                }
                Record::CallSign(parsed_call_sign) => {
                    if call_sign.is_some() {
                        return Err("Could not set call_sign (already defined)".to_string());
                    }
                    call_sign = Some(parsed_call_sign.to_string());
                }
                Record::TransponderCode(code) => {
                    if transponder_code.is_some() {
                        return Err("Could not set transponder_code (already defined)".to_string());
                    }
                    transponder_code = Some(code);
                }
                Record::ActivationTimes(parsed_times) => {
                    if activation_times.is_some() {
                        return Err("Could not set activation_times (already defined)".to_string());
                    }
                    activation_times = Some(parsed_times);
                }
                Record::VarX(coord) => {
                    var_x = Some(coord);
                }
                Record::VarD(direction) => {
                    var_d = Some(direction);
                }
                Record::Point(coord) => {
                    let segment = PolygonSegment::Point(coord);
                    match &mut geom {
                        None => {
                            geom = Some(Geometry::Polygon {
                                segments: vec![segment],
                            });
                        }
                        Some(Geometry::Polygon { segments }) => {
                            segments.push(segment);
                        }
                        Some(Geometry::Circle { .. }) => {
                            return Err("Cannot add a point to a circle".to_string());
                        }
                    }
                }
                Record::CircleRadius(radius) => match (&geom, &var_x) {
                    (None, Some(centerpoint)) => {
                        geom = Some(Geometry::Circle {
                            centerpoint: centerpoint.clone(),
                            radius,
                        });
                    }
                    (Some(_), _) => return Err("Geometry already set".to_string()),
                    (_, None) => return Err("Centerpoint missing".to_string()),
                },
                Record::ArcSegmentData {
                    radius,
                    angle_start,
                    angle_end,
                } => {
                    let centerpoint = var_x.clone().ok_or("Centerpoint missing".to_string())?;
                    let direction = var_d.unwrap_or_default();
                    let arc_segment = ArcSegment {
                        centerpoint,
                        radius,
                        angle_start,
                        angle_end,
                        direction,
                    };
                    let segment = PolygonSegment::ArcSegment(arc_segment);
                    match &mut geom {
                        None => {
                            geom = Some(Geometry::Polygon {
                                segments: vec![segment],
                            });
                        }
                        Some(Geometry::Polygon { segments }) => {
                            segments.push(segment);
                        }
                        Some(Geometry::Circle { .. }) => {
                            return Err("Cannot add a point to a circle".to_string());
                        }
                    }
                }
                Record::ArcData { start, end } => {
                    let centerpoint = var_x.clone().ok_or("Centerpoint missing".to_string())?;
                    let direction = var_d.unwrap_or_default();
                    let arc = Arc {
                        centerpoint,
                        start,
                        end,
                        direction,
                    };
                    let segment = PolygonSegment::Arc(arc);
                    match &mut geom {
                        None => {
                            geom = Some(Geometry::Polygon {
                                segments: vec![segment],
                            });
                        }
                        Some(Geometry::Polygon { segments }) => {
                            segments.push(segment);
                        }
                        Some(Geometry::Circle { .. }) => {
                            return Err("Cannot add a point to a circle".to_string());
                        }
                    }
                }
            }
        }
    }
}

impl<R: BufRead> Iterator for OpenAirIterator<R> {
    type Item = Result<Airspace, String>;

    fn next(&mut self) -> Option<Self::Item> {
        self.next_airspace().transpose()
    }
}

/// Process the reader until EOF, return an iterator over airspaces.
pub fn parse<R: BufRead>(reader: R) -> impl Iterator<Item = Result<Airspace, String>> {
    OpenAirIterator::new(reader)
}

/// Writes multiple airspaces in OpenAir format.
///
/// Airspaces are separated by blank lines.
pub fn write<'a, W: Write, I: IntoIterator<Item = &'a Airspace>>(
    mut writer: W,
    airspaces: I,
) -> std::io::Result<()> {
    for (i, airspace) in airspaces.into_iter().enumerate() {
        if i != 0 {
            // Write blank line between airspaces
            write!(writer, "\r\n")?;
        }
        airspace.write(&mut writer)?;
    }
    Ok(())
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
