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
#![allow(clippy::many_single_char_names)]
#![allow(clippy::non_ascii_literal)]

use std::fmt;
use std::io::BufRead;
use std::mem;
use std::sync::LazyLock;

use log::{debug, trace};
use regex::Regex;

#[cfg(feature = "serde")]
use serde::Serialize;

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

/// Altitude, either ground or a certain height AMSL in feet.
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(Serialize))]
#[cfg_attr(feature = "serde", serde(tag = "type", content = "val"))]
pub enum Altitude {
    /// Ground/surface level
    Gnd,
    /// Feet above mean sea level
    FeetAmsl(i32),
    /// Feet above ground level
    FeetAgl(i32),
    /// Flight level
    FlightLevel(u16),
    /// Unlimited
    Unlimited,
    /// Other (could not be parsed)
    Other(String),
}

impl fmt::Display for Altitude {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Self::Gnd => write!(f, "GND"),
            Self::FeetAmsl(ft) => write!(f, "{ft} ft AMSL"),
            Self::FeetAgl(ft) => write!(f, "{ft} ft AGL"),
            Self::FlightLevel(ft) => write!(f, "FL{ft}"),
            Self::Unlimited => write!(f, "Unlimited"),
            Self::Other(val) => write!(f, "?({val})"),
        }
    }
}

impl Altitude {
    #[allow(clippy::cast_possible_truncation)]
    fn m2ft(val: i32) -> Result<i32, &'static str> {
        if val > 654_553_015 {
            return Err("m2ft out of bounds (too large)");
        } else if val < -654_553_016 {
            return Err("m2ft out of bounds (too small)");
        }
        let m = f64::from(val);
        let feet = m / 0.3048;
        Ok(feet.round() as i32)
    }

    fn parse(data: &str) -> Result<Self, String> {
        match data {
            "gnd" | "Gnd" | "GND" | "sfc" | "Sfc" | "SFC" | "0" => {
                // Note: SFC = Surface. Seems to be another abbreviation for GND.
                Ok(Self::Gnd)
            }
            "unl" | "Unl" | "UNL" | "unlim" | "Unlim" | "UNLIM" | "unltd" | "Unltd" | "UNLTD"
            | "unlimited" | "Unlimited" | "UNLIMITED" => Ok(Self::Unlimited),
            fl if fl.starts_with("fl") || fl.starts_with("Fl") || fl.starts_with("FL") => {
                match fl[2..].trim().parse::<u16>() {
                    Ok(val) => Ok(Self::FlightLevel(val)),
                    Err(_) => Err(format!("Invalid altitude: {fl}")),
                }
            }
            other => {
                let is_digit = |c: &char| c.is_ascii_digit();
                let number: String = other.chars().take_while(is_digit).collect();
                let rest: String = other.chars().skip_while(is_digit).collect();

                static RE_FT_AMSL: LazyLock<Regex> =
                    LazyLock::new(|| Regex::new(r"(?i)^ft(:? a?msl)?$").unwrap());
                static RE_M_AMSL: LazyLock<Regex> =
                    LazyLock::new(|| Regex::new(r"(?i)^m(:?sl)?$").unwrap());
                static RE_FT_AGL: LazyLock<Regex> =
                    LazyLock::new(|| Regex::new(r"(?i)^(:?ft )?(:?agl|gnd|sfc)$").unwrap());
                static RE_M_AGL: LazyLock<Regex> =
                    LazyLock::new(|| Regex::new(r"(?i)^(:?m )?(:?agl|gnd|sfc)$").unwrap());

                if let Ok(val) = number.parse::<i32>() {
                    let trimmed = rest.trim();
                    if RE_FT_AMSL.is_match(trimmed) {
                        return Ok(Self::FeetAmsl(val));
                    } else if RE_FT_AGL.is_match(trimmed) {
                        return Ok(Self::FeetAgl(val));
                    } else if RE_M_AMSL.is_match(trimmed) {
                        return Ok(Self::FeetAmsl(Self::m2ft(val)?));
                    } else if RE_M_AGL.is_match(trimmed) {
                        return Ok(Self::FeetAgl(Self::m2ft(val)?));
                    }
                }
                Ok(Self::Other(other.to_string()))
            }
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

/// A coordinate pair (WGS84).
#[derive(Debug, PartialEq, Clone)]
#[cfg_attr(feature = "serde", derive(Serialize))]
pub struct Coord {
    pub lat: f64,
    pub lng: f64,
}

impl Coord {
    fn parse_number_opt(val: Option<&str>) -> Result<u16, ()> {
        val.and_then(|v| v.parse::<u16>().ok()).ok_or(())
    }

    #[allow(clippy::cast_possible_truncation, clippy::cast_possible_wrap)] // Impossible, since the RegEx limits length
    fn parse_component(val: &str) -> Result<f64, ()> {
        // Split by colon to separate degrees, minutes, and seconds
        let mut colon_parts = val.split(':');
        let deg = Self::parse_number_opt(colon_parts.next())?;

        // Get the minutes (decimal in DDM format or integer in DMS format)
        let raw_minutes = colon_parts.next().ok_or(())?;

        // Check if there's a third part (seconds in DMS format)
        //
        // See <https://github.com/naviter/seeyou_file_formats/blob/v2.1.2/OpenAir_File_Format_Support.md#geographic-position>
        if let Some(sec_part) = colon_parts.next() {
            // DMS format: DD:MM:SS or DD:MM:SS.fff
            let min = Self::parse_number_opt(Some(raw_minutes))?;
            let mut dot_parts = sec_part.split('.');
            let sec = Self::parse_number_opt(dot_parts.next())?;
            let mut total = f64::from(deg) + f64::from(min) / 60.0 + f64::from(sec) / 3600.0;

            // Handle fractional seconds if present
            if let Some(fractional) = dot_parts.next() {
                let frac = fractional.parse::<u16>().map_err(|_| ())?;
                total += f64::from(frac) / 10_f64.powi(fractional.len() as i32) / 3600.0;
            }
            Ok(total)
        } else if raw_minutes.contains('.') {
            // DDM format: DD:MM.mmm
            let decimal_minutes = raw_minutes.parse::<f64>().map_err(|_| ())?;
            let total = f64::from(deg) + decimal_minutes / 60.0;
            Ok(total)
        } else {
            // Invalid format
            Err(())
        }
    }

    fn multiplier_lat(val: &str) -> Result<f64, ()> {
        match val {
            "N" | "n" => Ok(1.0),
            "S" | "s" => Ok(-1.0),
            _ => Err(()),
        }
    }

    fn multiplier_lng(val: &str) -> Result<f64, ()> {
        match val {
            "E" | "e" => Ok(1.0),
            "W" | "w" => Ok(-1.0),
            _ => Err(()),
        }
    }

    fn parse(data: &str) -> Result<Self, String> {
        static RE: LazyLock<Regex> = LazyLock::new(|| {
            Regex::new(
                r"(?xi)
                ([0-9]{1,3}[\.:][0-9]{1,3}[\.:][0-9]{1,3}(:?\.?[0-9]{1,3})?)  # Lat
                \s*
                ([NS])                                    # North / South
                \s*,?\s*
                ([0-9]{1,3}[\.:][0-9]{1,3}[\.:][0-9]{1,3}(:?\.?[0-9]{1,3})?)  # Lon
                \s*
                ([EW])                                    # East / West
            ",
            )
            .unwrap()
        });
        let invalid = |_| format!("Invalid coord: \"{data}\"");
        let cap = RE
            .captures(data)
            .ok_or_else(|| format!("Invalid coord: \"{data}\""))?;
        let lat = Self::multiplier_lat(&cap[3]).map_err(invalid)?
            * Self::parse_component(&cap[1]).map_err(invalid)?;
        let lng = Self::multiplier_lng(&cap[6]).map_err(invalid)?
            * Self::parse_component(&cap[4]).map_err(invalid)?;
        Ok(Self { lat, lng })
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

    use indoc::indoc;

    mod coord {
        use super::*;

        #[test]
        #[allow(clippy::unreadable_literal)]
        fn parse_valid() {
            // With spaces
            assert_eq!(
                Coord::parse("46:51:44 N 009:19:42 E"),
                Ok(Coord {
                    lat: 46.86222222222222,
                    lng: 9.328333333333333
                })
            );

            // Without spaces
            assert_eq!(
                Coord::parse("46:51:44N 009:19:42E"),
                Ok(Coord {
                    lat: 46.86222222222222,
                    lng: 9.328333333333333
                })
            );

            // DDM format (degrees and decimal minutes)
            assert_eq!(
                Coord::parse("46:51.44 N 009:19.42 E"),
                Ok(Coord {
                    lat: 46.85733333333334,
                    lng: 9.323666666666666
                })
            );

            // South / west
            assert_eq!(
                Coord::parse("46:51:44 S 009:19:42 W"),
                Ok(Coord {
                    lat: -46.86222222222222,
                    lng: -9.328333333333333
                })
            );

            // Fractional part
            assert_eq!(
                Coord::parse("1:0:0.123 N 2:0:1.2 E"),
                Ok(Coord {
                    lat: 1.0 + 0.123 / 3600.0,
                    lng: 2.0 + 1.2 / 3600.0
                })
            );

            // Comma in between
            assert!(Coord::parse("45:42:21 N, 000:38:41 W").is_ok());

            // Lowercase letters
            assert!(Coord::parse("49:33:8 n 5:47:37 e").is_ok());
        }

        #[test]
        fn parse_invalid() {
            assert_eq!(
                Coord::parse("46:51:44 Q 009:19:42 R"),
                Err("Invalid coord: \"46:51:44 Q 009:19:42 R\"".to_string())
            );
            assert_eq!(
                Coord::parse("46x51x44 S 009x19x42 W"),
                Err("Invalid coord: \"46x51x44 S 009x19x42 W\"".to_string())
            );
        }
    }

    mod altitude {
        use super::*;

        #[test]
        fn m2ft() {
            assert_eq!(Altitude::m2ft(0).unwrap(), 0);
            assert_eq!(Altitude::m2ft(1).unwrap(), 3);
            assert_eq!(Altitude::m2ft(2).unwrap(), 7);
            assert_eq!(Altitude::m2ft(100).unwrap(), 328);
            assert_eq!(Altitude::m2ft(654_553_015).unwrap(), 2_147_483_645);
            assert_eq!(Altitude::m2ft(-654_553_016).unwrap(), -2_147_483_648);
            assert!(Altitude::m2ft(654_553_016).is_err());
            assert!(Altitude::m2ft(-654_553_017).is_err());
        }

        #[test]
        fn parse_gnd() {
            assert_eq!(Altitude::parse("gnd").unwrap(), Altitude::Gnd);
            assert_eq!(Altitude::parse("Gnd").unwrap(), Altitude::Gnd);
            assert_eq!(Altitude::parse("GND").unwrap(), Altitude::Gnd);
            assert_eq!(Altitude::parse("sfc").unwrap(), Altitude::Gnd);
            assert_eq!(Altitude::parse("Sfc").unwrap(), Altitude::Gnd);
            assert_eq!(Altitude::parse("SFC").unwrap(), Altitude::Gnd);
        }

        #[test]
        fn parse_amsl() {
            assert_eq!(Altitude::parse("42 ft").unwrap(), Altitude::FeetAmsl(42));
            assert_eq!(Altitude::parse("42 FT").unwrap(), Altitude::FeetAmsl(42));
            assert_eq!(Altitude::parse("42ft").unwrap(), Altitude::FeetAmsl(42));
            assert_eq!(Altitude::parse("42  ft").unwrap(), Altitude::FeetAmsl(42));
            assert_eq!(
                Altitude::parse("42 ft AMSL").unwrap(),
                Altitude::FeetAmsl(42)
            );
        }

        #[test]
        fn parse_agl() {
            assert_eq!(Altitude::parse("42 ft agl").unwrap(), Altitude::FeetAgl(42));
            assert_eq!(Altitude::parse("42FT Agl").unwrap(), Altitude::FeetAgl(42));
            assert_eq!(Altitude::parse("42 ft GND").unwrap(), Altitude::FeetAgl(42));
            assert_eq!(Altitude::parse("42 GND").unwrap(), Altitude::FeetAgl(42));
            assert_eq!(Altitude::parse("42SFC").unwrap(), Altitude::FeetAgl(42));
        }

        #[test]
        fn parse_fl() {
            assert_eq!(Altitude::parse("fl50").unwrap(), Altitude::FlightLevel(50));
            assert_eq!(
                Altitude::parse("FL 180").unwrap(),
                Altitude::FlightLevel(180)
            );
            assert_eq!(
                Altitude::parse("FL130").unwrap(),
                Altitude::FlightLevel(130)
            );
        }
    }

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

    mod parse_airspace {
        use super::*;

        /// Parse an airspace as generated by flyland.
        #[test]
        fn flyland_buochs() {
            let mut airspace = indoc!(
                "
                AC D
                AN BUOCHS Be CTR 119.625
                AL GND
                AH 12959 ft
                DP 46:57:13 N 008:27:52 E
                DP 46:57:46 N 008:30:41 E
                DP 46:57:55 N 008:28:40 E
                DP 46:58:28 N 008:27:56 E
                DP 46:57:13 N 008:27:52 E
                * n-Points: 5
            "
            )
            .as_bytes();
            let mut spaces = parse(&mut airspace).unwrap();
            assert_eq!(spaces.len(), 1);
            let space: Airspace = spaces.pop().unwrap();
            assert_eq!(space.name, "BUOCHS Be CTR 119.625");
            assert_eq!(space.lower_bound, Altitude::Gnd);
            assert_eq!(space.upper_bound, Altitude::FeetAmsl(12959));
            if let Geometry::Polygon { segments } = space.geom {
                assert_eq!(segments.len(), 5);
            } else {
                panic!("Unexpected enum variant");
            }
        }

        /// The order of bounds should not matter.
        #[test]
        fn inverted_bounds() {
            let mut a1 = indoc!(
                "
                AC D
                AN SOMESPACE
                AL GND
                AH 12959 ft
                DP 46:57:13 N 008:27:52 E
                DP 46:57:46 N 008:30:41 E
                *
            "
            )
            .as_bytes();
            let mut a2 = indoc!(
                "
                AC D
                AN SOMESPACE
                AH 12959 ft
                AL GND
                DP 46:57:13 N 008:27:52 E
                DP 46:57:46 N 008:30:41 E
                *
            "
            )
            .as_bytes();
            let space1 = parse(&mut a1).unwrap().pop().unwrap();
            let space2 = parse(&mut a2).unwrap().pop().unwrap();
            assert_eq!(space1, space2);
        }

        /// Variables can be defined multiple times.
        #[test]
        fn multi_variable() {
            let mut a = indoc!(
                "
                AC D
                AN SOMESPACE
                AL GND
                AH FL100
                V X=52:00:00N 013:00:00E
                V D=+
                DA 2,0,30
                V X=52:00:00N 013:00:00E
                V D=-
                DA 4,60,30
                *
            "
            )
            .as_bytes();
            let airspace = parse(&mut a).unwrap().pop().unwrap();
            assert_eq!(
                airspace.geom,
                Geometry::Polygon {
                    segments: vec![
                        PolygonSegment::ArcSegment(ArcSegment {
                            centerpoint: Coord {
                                lat: 52.0,
                                lng: 13.0
                            },
                            radius: 2.0,
                            angle_start: 0.0,
                            angle_end: 30.0,
                            direction: Direction::Cw,
                        }),
                        PolygonSegment::ArcSegment(ArcSegment {
                            centerpoint: Coord {
                                lat: 52.0,
                                lng: 13.0
                            },
                            radius: 4.0,
                            angle_start: 60.0,
                            angle_end: 30.0,
                            direction: Direction::Ccw,
                        }),
                    ],
                }
            );
        }

        /// Test AY/AF/AG records.
        #[test]
        fn extension_records() {
            let mut a = indoc!(
                "
                AC D
                AN SOMESPACE
                AL GND
                AH 100 ft AGL
                AY AWY
                AF 132.350
                AG Dutch Mil
                AX 1234
                A* custom extension
                V X=52:00:00 N 013:00:00 E
                DC 5
            "
            )
            .as_bytes();
            let airspace = parse(&mut a).unwrap().pop().unwrap();
            assert_eq!(airspace.type_, Some("AWY".to_string()));
            assert_eq!(airspace.frequency, Some("132.350".to_string()));
            assert_eq!(airspace.call_sign, Some("Dutch Mil".to_string()));
            assert_eq!(airspace.transponder_code, Some(1234));
        }
    }

    #[cfg(feature = "serde")]
    mod serde {
        use super::*;
        use serde_json::to_string;

        #[test]
        fn serialize_json() {
            let airspace = Airspace {
                name: "SUPERSPACE".into(),
                class: Class::Prohibited,
                lower_bound: Altitude::Gnd,
                upper_bound: Altitude::FeetAgl(3000),
                geom: Geometry::Polygon {
                    segments: vec![
                        PolygonSegment::Point(Coord { lat: 1.0, lng: 2.0 }),
                        PolygonSegment::Point(Coord { lat: 1.1, lng: 2.0 }),
                        PolygonSegment::Arc(Arc {
                            centerpoint: Coord {
                                lat: 1.05,
                                lng: 2.05,
                            },
                            start: Coord { lat: 1.1, lng: 2.0 },
                            end: Coord { lat: 1.0, lng: 2.1 },
                            direction: Direction::Cw,
                        }),
                        PolygonSegment::ArcSegment(ArcSegment {
                            centerpoint: Coord { lat: 3.0, lng: 3.0 },
                            radius: 1.5,
                            angle_start: 30.0,
                            angle_end: 45.0,
                            direction: Direction::Ccw,
                        }),
                        PolygonSegment::Point(Coord { lat: 1.0, lng: 2.0 }),
                    ],
                },
                type_: None,
                frequency: None,
                call_sign: None,
                transponder_code: None,
            };
            assert_eq!(
                to_string(&airspace).unwrap(),
                "{\"name\":\"SUPERSPACE\",\
                  \"class\":\"Prohibited\",\
                  \"lowerBound\":{\"type\":\"Gnd\"},\
                  \"upperBound\":{\"type\":\"FeetAgl\",\"val\":3000},\
                  \"geom\":{\
                    \"type\":\"Polygon\",\
                    \"segments\":[\
                      {\"type\":\"Point\",\"lat\":1.0,\"lng\":2.0},\
                      {\"type\":\"Point\",\"lat\":1.1,\"lng\":2.0},\
                      {\"type\":\"Arc\",\
                       \"centerpoint\":{\"lat\":1.05,\"lng\":2.05},\
                       \"start\":{\"lat\":1.1,\"lng\":2.0},\
                       \"end\":{\"lat\":1.0,\"lng\":2.1},\
                       \"direction\":\"cw\"},\
                      {\"type\":\"ArcSegment\",\
                       \"centerpoint\":{\"lat\":3.0,\"lng\":3.0},\
                       \"radius\":1.5,\
                       \"angleStart\":30.0,\
                       \"angleEnd\":45.0,\
                       \"direction\":\"ccw\"},\
                      {\"type\":\"Point\",\"lat\":1.0,\"lng\":2.0}\
                    ]\
                  }\
                 }"
            );
        }

        #[test]
        fn serialize_json_ctr() {
            let airspace = Airspace {
                name: "Control Zone".into(),
                class: Class::Ctr,
                lower_bound: Altitude::Gnd,
                upper_bound: Altitude::FeetAgl(1000),
                geom: Geometry::Polygon { segments: vec![] },
                type_: None,
                frequency: None,
                call_sign: None,
                transponder_code: None,
            };
            assert_eq!(
                to_string(&airspace).unwrap(),
                "{\"name\":\"Control Zone\",\
                  \"class\":\"CTR\",\
                  \"lowerBound\":{\"type\":\"Gnd\"},\
                  \"upperBound\":{\"type\":\"FeetAgl\",\"val\":1000},\
                  \"geom\":{\
                    \"type\":\"Polygon\",\
                    \"segments\":[]\
                  }\
                 }"
            );
        }
    }
}
