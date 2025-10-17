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

mod altitude;
mod coords;

use std::{fmt, io::BufRead, mem};

use log::{debug, trace};
#[cfg(feature = "serde")]
use serde::Serialize;

pub use crate::{altitude::Altitude, coords::Coord};

/// Airspace class.
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub enum Class {
    /// Airspace A
    A,
    /// Airspace B
    B,
    /// Airspace C
    C,
    /// Airspace D
    D,
    /// Airspace E
    E,
    /// Airspace F
    F,
    /// Airspace G
    G,
    /// Controlled Traffic Region
    #[cfg_attr(feature = "serde", serde(rename = "CTR"))]
    Ctr,
    /// Restricted area
    Restricted,
    /// Danger area
    Danger,
    /// Prohibited area
    Prohibited,
    /// Prohibited for gliders
    GliderProhibited,
    /// Wave window
    WaveWindow,
    /// Radio mandatory zone
    RadioMandatoryZone,
    /// Transponder mandatory zone
    TransponderMandatoryZone,
    /// Unclassified
    Unclassified,
}

impl fmt::Display for Class {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl Class {
    fn parse(data: &str) -> Result<Self, String> {
        match data {
            "A" => Ok(Self::A),
            "B" => Ok(Self::B),
            "C" => Ok(Self::C),
            "D" => Ok(Self::D),
            "E" => Ok(Self::E),
            "F" => Ok(Self::F),
            "G" => Ok(Self::G),
            "CTR" => Ok(Self::Ctr),
            "R" => Ok(Self::Restricted),
            "Q" => Ok(Self::Danger),
            "P" => Ok(Self::Prohibited),
            "GP" => Ok(Self::GliderProhibited),
            "W" => Ok(Self::WaveWindow),
            "RMZ" => Ok(Self::RadioMandatoryZone),
            "TMZ" => Ok(Self::TransponderMandatoryZone),
            "UNC" => Ok(Self::Unclassified),
            other => Err(format!("Invalid class: {other}")),
        }
    }
}

/// Arc direction, either clockwise or counterclockwise.
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "lowercase"))]
pub enum Direction {
    /// Clockwise.
    Cw,
    /// Counterclockwise.
    Ccw,
}

impl Default for Direction {
    fn default() -> Self {
        Self::Cw
    }
}

impl Direction {
    fn parse(data: &str) -> Result<Self, String> {
        match data {
            "+" => Ok(Self::Cw),
            "-" => Ok(Self::Ccw),
            _ => Err(format!("Invalid direction: {data}")),
        }
    }
}

/// An arc segment (DA record).
#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(rename_all = "camelCase"))]
pub struct ArcSegment {
    pub centerpoint: Coord,
    pub radius: f32,
    pub angle_start: f32,
    pub angle_end: f32,
    pub direction: Direction,
}

impl ArcSegment {
    /// Return the angle if it's in the range 0..360, or an error otherwise.
    fn validate_angle(val: f32) -> Result<f32, String> {
        if val > 360.0 {
            return Err(format!("Angle {val} too large"));
        }
        if val < 0.0 {
            return Err(format!("Angle {val} is negative"));
        }
        Ok(val)
    }

    fn parse(data: &str, centerpoint: Coord, direction: Direction) -> Result<Self, String> {
        let errmsg = || format!("Invalid arc segment data: {data}");
        let parts: Vec<f32> = data
            .split(',')
            .map(str::trim)
            .map(str::parse)
            .collect::<Result<Vec<f32>, _>>()
            .map_err(|_| errmsg())?;
        if parts.len() != 3 {
            return Err(errmsg());
        }
        Ok(Self {
            centerpoint,
            radius: parts[0],
            angle_start: Self::validate_angle(parts[1])?,
            angle_end: Self::validate_angle(parts[2])?,
            direction,
        })
    }
}

/// An arc (DB record).
#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub struct Arc {
    pub centerpoint: Coord,
    pub start: Coord,
    pub end: Coord,
    pub direction: Direction,
}

impl Arc {
    fn parse(data: &str, centerpoint: Coord, direction: Direction) -> Result<Self, String> {
        let errmsg = || format!("Invalid arc data: {data}");
        let parts: Vec<Coord> = data
            .split(',')
            .map(str::trim)
            .map(Coord::parse)
            .collect::<Result<Vec<Coord>, _>>()
            .map_err(|_| errmsg())?;
        if parts.len() != 2 {
            return Err(errmsg());
        }
        let mut coords = parts.into_iter();
        Ok(Self {
            centerpoint,
            start: coords.next().unwrap(),
            end: coords.next().unwrap(),
            direction,
        })
    }
}

/// A polygon segment.
#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(tag = "type"))]
pub enum PolygonSegment {
    Point(Coord),
    Arc(Arc),
    ArcSegment(ArcSegment),
}

#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(tag = "type"))]
pub enum Geometry {
    Polygon {
        /// Segments describing the polygon.
        ///
        /// The polygon may be open or closed.
        segments: Vec<PolygonSegment>,
    },
    Circle {
        /// The centerpoint of the circle.
        centerpoint: Coord,
        /// Radius of the circle in nautical miles (1 NM = 1852 m).
        radius: f32,
    },
}

impl fmt::Display for Geometry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Polygon { segments } => write!(f, "Polygon[{}]", segments.len()),
            Self::Circle { radius, .. } => write!(f, "Circle[r={radius}NM]"),
        }
    }
}

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
            Some(Geometry::Polygon { ref mut segments }) => {
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
        })
    }
}

/// Return whether this line contains the start of a new airspace.
#[inline]
fn starts_airspace(line: &str) -> bool {
    line.starts_with("AC ")
}

/// Process a line.
fn process(builder: &mut AirspaceBuilder, line: &str) -> Result<(), String> {
    if line.trim().is_empty() {
        trace!("Empty line, ignoring");
        return Ok(());
    }

    let mut chars = line.chars().filter(|c: &char| !c.is_ascii_whitespace());
    let t1 = chars.next().ok_or_else(|| "Line too short".to_string())?;
    let t2 = chars.next().unwrap_or(' ');
    let data = line.split_once(' ').map(|x| x.1).unwrap_or("").trim();

    trace!("Input: \"{:1}{:1}\"", t1, t2);
    match (t1, t2) {
        ('*', _) => trace!("-> Comment, ignore"),
        ('A', 'C') => {
            // Airspace class
            let class = Class::parse(data)?;
            trace!("-> Found class: {}", class);
            builder.set_class(class)?;
        }
        ('A', 'N') => {
            trace!("-> Found name: {}", data);
            builder.set_name(data.to_string())?;
        }
        ('A', 'L') => {
            let altitude = Altitude::parse(data)?;
            trace!("-> Found lower bound: {}", altitude);
            builder.set_lower_bound(altitude)?;
        }
        ('A', 'H') => {
            let altitude = Altitude::parse(data)?;
            trace!("-> Found upper bound: {}", altitude);
            builder.set_upper_bound(altitude)?;
        }
        ('A', 'T') => {
            trace!("-> Label placement hint, ignore");
        }
        ('A', 'Y') => {
            trace!("-> Found type: {}", data);
            builder.set_type(data.to_string())?;
        }
        ('A', 'F') => {
            trace!("-> Found frequency: {}", data);
            builder.set_frequency(data.to_string())?;
        }
        ('A', 'G') => {
            trace!("-> Found call sign: {}", data);
            builder.set_call_sign(data.to_string())?;
        }
        ('A', 'X') => {
            let transponder_code = data
                .parse()
                .map_err(|_| format!("Invalid transponder code: {}", data))?;
            trace!("-> Found transponder code: {}", transponder_code);
            builder.set_transponder_code(transponder_code)?;
        }
        ('A', _) => trace!("-> Found unknown extension record: {}", line),
        ('S', 'P') => trace!("-> Pen, ignore"),
        ('S', 'B') => trace!("-> Brush, ignore"),
        ('V', 'X') => {
            trace!("-> Found X variable");
            let coord = Coord::parse(data.get(2..).unwrap_or(""))?;
            builder.set_var_x(coord);
        }
        ('V', 'D') => {
            trace!("-> Found D variable");
            let direction = Direction::parse(data.get(2..).unwrap_or(""))?;
            builder.set_var_d(direction);
        }
        ('D', 'P') => {
            trace!("-> Found point");
            let coord = Coord::parse(data)?;
            builder.add_segment(PolygonSegment::Point(coord))?;
        }
        ('D', 'C') => {
            trace!("-> Found circle radius");
            let radius = data
                .parse::<f32>()
                .map_err(|_| format!("Invalid radius: {data}"))?;
            builder.set_circle_radius(radius)?;
        }
        ('D', 'A') => {
            trace!("-> Found arc segment");
            let centerpoint = builder.var_x.clone().ok_or("Centerpoint missing")?;
            let direction = builder.var_d.unwrap_or_default();
            let arc_segment = ArcSegment::parse(data, centerpoint, direction)?;
            builder.add_segment(PolygonSegment::ArcSegment(arc_segment))?;
        }
        ('D', 'B') => {
            trace!("-> Found arc");
            let centerpoint = builder.var_x.clone().ok_or("Centerpoint missing")?;
            let direction = builder.var_d.unwrap_or_default();
            let arc = Arc::parse(data, centerpoint, direction)?;
            builder.add_segment(PolygonSegment::Arc(arc))?;
        }
        (t1, t2) => return Err(format!("Parse error (unexpected \"{t1:1}{t2:1}\")")),
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

        // Trim BOM and whitespace
        let trimmed_line = line.trim_start_matches('\u{feff}').trim();

        // Determine whether we reached the start of a new airspace
        let start_of_airspace = starts_airspace(trimmed_line);

        // A new airspace starts, collect the old one first
        if start_of_airspace && !builder.new {
            let old_builder = mem::replace(&mut builder, AirspaceBuilder::new());
            airspaces.push(old_builder.finish()?);
        }

        // Process current line
        process(&mut builder, trimmed_line)?;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    mod arc_segment {
        use super::*;

        static COORD: Coord = Coord { lat: 1.0, lng: 2.0 };

        #[test]
        fn parse_ok() {
            assert_eq!(
                ArcSegment::parse("10,270,290", COORD.clone(), Direction::Cw).unwrap(),
                ArcSegment {
                    centerpoint: COORD.clone(),
                    radius: 10.0,
                    angle_start: 270.0,
                    angle_end: 290.0,
                    direction: Direction::Cw,
                }
            );
            assert_eq!(
                ArcSegment::parse("23,0,30", COORD.clone(), Direction::Ccw).unwrap(),
                ArcSegment {
                    centerpoint: COORD.clone(),
                    radius: 23.0,
                    angle_start: 0.0,
                    angle_end: 30.0,
                    direction: Direction::Ccw,
                }
            );
        }

        #[test]
        fn parse_with_spaces() {
            assert_eq!(
                ArcSegment::parse(" 10 ,    270 ,290", COORD.clone(), Direction::Cw).unwrap(),
                ArcSegment {
                    centerpoint: COORD.clone(),
                    radius: 10.0,
                    angle_start: 270.0,
                    angle_end: 290.0,
                    direction: Direction::Cw,
                }
            );
        }

        #[test]
        fn parse_invalid_too_many() {
            assert!(ArcSegment::parse(" 10 ,    270 ,290,", COORD.clone(), Direction::Cw).is_err());
        }

        #[test]
        fn parse_invalid_angle_too_large() {
            assert!(ArcSegment::parse("10,270,361", COORD.clone(), Direction::Cw).is_err());
        }

        #[test]
        fn parse_invalid_angle_negative() {
            assert!(ArcSegment::parse("10,270,-10", COORD.clone(), Direction::Cw).is_err());
        }
    }
}
