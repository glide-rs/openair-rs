use std::io::Write;

use log::trace;

use crate::{
    activations::ActivationTimes, altitude::Altitude, classes::Class, coords::Coord,
    geometry::Direction,
};

/// Validate an angle is in the range 0..360.
fn validate_angle(val: f32) -> Result<f32, String> {
    if val > 360.0 {
        return Err(format!("Angle {val} too large"));
    }
    if val < 0.0 {
        return Err(format!("Angle {val} is negative"));
    }
    Ok(val)
}

/// A parsed OpenAir record from a single line.
#[derive(Debug, PartialEq)]
pub enum Record<'a> {
    // Airspace base records
    AirspaceClass(Class),
    AirspaceName(&'a str),
    LowerBound(Altitude),
    UpperBound(Altitude),

    // Extension records
    AirspaceType(&'a str),
    Frequency(&'a str),
    CallSign(&'a str),
    TransponderCode(u16),
    ActivationTimes(ActivationTimes),
    UnknownExtension(&'a str),

    // Variable records
    VarX(Coord),
    VarD(Direction),

    // Geometry records
    Point(Coord),
    CircleRadius(f32),
    ArcSegmentData {
        radius: f32,
        angle_start: f32,
        angle_end: f32,
    },
    ArcData {
        start: Coord,
        end: Coord,
    },

    // Ignored records (no payload)
    Empty,
    Comment,
    LabelPlacement,
    Pen,
    Brush,
}

impl<'a> Record<'a> {
    /// Returns true if this record is a header record (A* records that define airspace properties).
    /// Header records include AC, AN, AL, AH, AY, AF, AG, AX, AA, and unknown A* extensions.
    pub fn is_header(&self) -> bool {
        matches!(
            self,
            Record::AirspaceClass(_)
                | Record::AirspaceName(_)
                | Record::LowerBound(_)
                | Record::UpperBound(_)
                | Record::AirspaceType(_)
                | Record::Frequency(_)
                | Record::CallSign(_)
                | Record::TransponderCode(_)
                | Record::ActivationTimes(_)
                | Record::UnknownExtension(_)
        )
    }

    /// Writes the record in OpenAir format with CRLF line ending.
    pub fn write<W: Write>(self, mut writer: W) -> std::io::Result<()> {
        match self {
            Record::AirspaceClass(class) => write!(writer, "AC {}\r\n", class.to_str()),
            Record::AirspaceType(ty) => write!(writer, "AY {ty}\r\n"),
            Record::AirspaceName(name) => write!(writer, "AN {name}\r\n"),
            Record::LowerBound(alt) => {
                write!(writer, "AL ")?;
                alt.write(&mut writer)?;
                write!(writer, "\r\n")
            }
            Record::UpperBound(alt) => {
                write!(writer, "AH ")?;
                alt.write(&mut writer)?;
                write!(writer, "\r\n")
            }
            Record::Frequency(freq) => write!(writer, "AF {freq}\r\n"),
            Record::CallSign(sign) => write!(writer, "AG {sign}\r\n"),
            Record::TransponderCode(code) => write!(writer, "AX {code}\r\n"),
            Record::ActivationTimes(times) => {
                write!(writer, "AA ")?;
                times.write(&mut writer)?;
                write!(writer, "\r\n")
            }
            Record::UnknownExtension(ext) => write!(writer, "{ext}\r\n"),
            Record::VarX(coord) => {
                write!(writer, "V X=")?;
                coord.write(&mut writer)?;
                write!(writer, "\r\n")
            }
            Record::VarD(direction) => {
                let dir_str = match direction {
                    Direction::Cw => "+",
                    Direction::Ccw => "-",
                };
                write!(writer, "V D={dir_str}\r\n")
            }
            Record::Point(coord) => {
                write!(writer, "DP ")?;
                coord.write(&mut writer)?;
                write!(writer, "\r\n")
            }
            Record::CircleRadius(radius) => write!(writer, "DC {radius}\r\n"),
            Record::ArcSegmentData {
                radius,
                angle_start,
                angle_end,
            } => write!(writer, "DA {radius}, {angle_start}, {angle_end}\r\n"),
            Record::ArcData { start, end } => {
                write!(writer, "DB ")?;
                start.write(&mut writer)?;
                write!(writer, ", ")?;
                end.write(&mut writer)?;
                write!(writer, "\r\n")
            }
            Record::Empty => write!(writer, "\r\n"),
            Record::Comment | Record::LabelPlacement | Record::Pen | Record::Brush => {
                // These records are ignored when writing
                Ok(())
            }
        }
    }

    pub fn parse(line: &'a str) -> Result<Self, String> {
        let trimmed = line.trim();

        // Check for empty lines
        if trimmed.is_empty() {
            return Ok(Record::Empty);
        }

        // Extract record type (two characters)
        let mut chars = trimmed.chars().filter(|c: &char| !c.is_ascii_whitespace());
        let t1 = chars.next().ok_or_else(|| "Line too short".to_string())?;
        let t2 = chars.next().unwrap_or(' ');
        let data = trimmed.split_once(' ').map(|x| x.1).unwrap_or("").trim();

        trace!("Input: \"{:1}{:1}\"", t1, t2);
        match (t1, t2) {
            ('*', _) => {
                trace!("-> Comment, ignore");
                Ok(Record::Comment)
            }
            ('A', 'C') => {
                // Airspace class
                let class = Class::parse(data)?;
                trace!("-> Found class: {}", class);
                Ok(Record::AirspaceClass(class))
            }
            ('A', 'N') => {
                trace!("-> Found name: {}", data);
                Ok(Record::AirspaceName(data))
            }
            ('A', 'L') => {
                let altitude = Altitude::parse(data)?;
                trace!("-> Found lower bound: {}", altitude);
                Ok(Record::LowerBound(altitude))
            }
            ('A', 'H') => {
                let altitude = Altitude::parse(data)?;
                trace!("-> Found upper bound: {}", altitude);
                Ok(Record::UpperBound(altitude))
            }
            ('A', 'T') => {
                trace!("-> Label placement hint, ignore");
                Ok(Record::LabelPlacement)
            }
            ('A', 'Y') => {
                trace!("-> Found type: {}", data);
                Ok(Record::AirspaceType(data))
            }
            ('A', 'F') => {
                trace!("-> Found frequency: {}", data);
                Ok(Record::Frequency(data))
            }
            ('A', 'G') => {
                trace!("-> Found call sign: {}", data);
                Ok(Record::CallSign(data))
            }
            ('A', 'X') => {
                let transponder_code = data
                    .parse()
                    .map_err(|_| format!("Invalid transponder code: {}", data))?;
                trace!("-> Found transponder code: {}", transponder_code);
                Ok(Record::TransponderCode(transponder_code))
            }
            ('A', 'A') => {
                let activation_times = data.parse()?;
                trace!("-> Found activation times: {:?}", activation_times);
                Ok(Record::ActivationTimes(activation_times))
            }
            ('A', _) => {
                trace!("-> Found unknown extension record: {}", trimmed);
                Ok(Record::UnknownExtension(trimmed))
            }
            ('S', 'P') => {
                trace!("-> Pen, ignore");
                Ok(Record::Pen)
            }
            ('S', 'B') => {
                trace!("-> Brush, ignore");
                Ok(Record::Brush)
            }
            ('V', 'X') => {
                trace!("-> Found X variable");
                let coord = Coord::parse(data.get(2..).unwrap_or(""))?;
                Ok(Record::VarX(coord))
            }
            ('V', 'D') => {
                trace!("-> Found D variable");
                let direction = Direction::parse(data.get(2..).unwrap_or(""))?;
                Ok(Record::VarD(direction))
            }
            ('D', 'P') => {
                trace!("-> Found point");
                let coord = Coord::parse(data)?;
                Ok(Record::Point(coord))
            }
            ('D', 'C') => {
                trace!("-> Found circle radius");
                let radius = data
                    .parse::<f32>()
                    .map_err(|_| format!("Invalid radius: {data}"))?;
                Ok(Record::CircleRadius(radius))
            }
            ('D', 'A') => {
                trace!("-> Found arc segment");
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
                let radius = parts[0];
                let angle_start = validate_angle(parts[1])?;
                let angle_end = validate_angle(parts[2])?;
                Ok(Record::ArcSegmentData {
                    radius,
                    angle_start,
                    angle_end,
                })
            }
            ('D', 'B') => {
                trace!("-> Found arc");
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
                Ok(Record::ArcData {
                    start: coords.next().unwrap(),
                    end: coords.next().unwrap(),
                })
            }
            (t1, t2) => Err(format!("Parse error (unexpected \"{t1:1}{t2:1}\")")),
        }
    }
}

#[cfg(test)]
mod tests {
    use insta::assert_compact_debug_snapshot;

    use super::*;

    #[test]
    fn parse_arc_segment_ok() {
        assert_compact_debug_snapshot!(
            Record::parse("DA 10,270,290"),
            @r#"Ok(ArcSegmentData { radius: 10.0, angle_start: 270.0, angle_end: 290.0 })"#,
        );

        assert_compact_debug_snapshot!(
            Record::parse("DA 23,0,30"),
            @r#"Ok(ArcSegmentData { radius: 23.0, angle_start: 0.0, angle_end: 30.0 })"#,
        );
    }

    #[test]
    fn parse_arc_segment_with_spaces() {
        assert_compact_debug_snapshot!(
            Record::parse("DA  10 ,    270 ,290"),
            @r#"Ok(ArcSegmentData { radius: 10.0, angle_start: 270.0, angle_end: 290.0 })"#,
        );
    }

    #[test]
    fn parse_arc_segment_invalid_too_many() {
        assert_compact_debug_snapshot!(
            Record::parse("DA  10 ,    270 ,290,"),
            @r#"Err("Invalid arc segment data: 10 ,    270 ,290,")"#,
        );
    }

    #[test]
    fn parse_arc_segment_invalid_angle_too_large() {
        assert_compact_debug_snapshot!(
            Record::parse("DA 10,270,361"),
            @r#"Err("Angle 361 too large")"#,
        );
    }

    #[test]
    fn parse_arc_segment_invalid_angle_negative() {
        assert_compact_debug_snapshot!(
            Record::parse("DA 10,270,-10"),
            @r#"Err("Angle -10 is negative")"#,
        );
    }

    fn write_record(record: Record) -> String {
        let mut buf = Vec::new();
        record.write(&mut buf).unwrap();
        String::from_utf8(buf).unwrap()
    }

    #[test]
    fn write_airspace_class() {
        assert_eq!(write_record(Record::AirspaceClass(Class::A)), "AC A\r\n");
        assert_eq!(
            write_record(Record::AirspaceClass(Class::Ctr)),
            "AC CTR\r\n"
        );
    }

    #[test]
    fn write_airspace_type() {
        assert_eq!(write_record(Record::AirspaceType("MATZ")), "AY MATZ\r\n");
    }

    #[test]
    fn write_airspace_name() {
        assert_eq!(
            write_record(Record::AirspaceName("Test Zone")),
            "AN Test Zone\r\n"
        );
    }

    #[test]
    fn write_bounds() {
        assert_eq!(
            write_record(Record::LowerBound(Altitude::Gnd)),
            "AL GND\r\n"
        );
        assert_eq!(
            write_record(Record::UpperBound(Altitude::FlightLevel(195))),
            "AH FL195\r\n"
        );
    }

    #[test]
    fn write_frequency() {
        assert_eq!(write_record(Record::Frequency("123.45")), "AF 123.45\r\n");
    }

    #[test]
    fn write_call_sign() {
        assert_eq!(write_record(Record::CallSign("TOWER")), "AG TOWER\r\n");
    }

    #[test]
    fn write_transponder_code() {
        assert_eq!(write_record(Record::TransponderCode(7000)), "AX 7000\r\n");
    }

    #[test]
    fn write_activation_times() {
        let times = "2023-12-16T12:00Z/2023-12-16T13:00Z".parse().unwrap();
        assert_eq!(
            write_record(Record::ActivationTimes(times)),
            "AA 2023-12-16T12:00:00.0+00:00/2023-12-16T13:00:00.0+00:00\r\n"
        );
    }

    #[test]
    fn write_unknown_extension() {
        assert_eq!(
            write_record(Record::UnknownExtension("AZ custom data")),
            "AZ custom data\r\n"
        );
    }

    #[test]
    fn write_var_x() {
        let coord = Coord {
            lat: 46.86222222222222,
            lng: 9.328333333333333,
        };
        assert_eq!(
            write_record(Record::VarX(coord)),
            "V X=46:51:44 N 009:19:42 E\r\n"
        );
    }

    #[test]
    fn write_var_d() {
        assert_eq!(write_record(Record::VarD(Direction::Cw)), "V D=+\r\n");
        assert_eq!(write_record(Record::VarD(Direction::Ccw)), "V D=-\r\n");
    }

    #[test]
    fn write_point() {
        let coord = Coord {
            lat: 46.86222222222222,
            lng: 9.328333333333333,
        };
        assert_eq!(
            write_record(Record::Point(coord)),
            "DP 46:51:44 N 009:19:42 E\r\n"
        );
    }

    #[test]
    fn write_circle_radius() {
        assert_eq!(write_record(Record::CircleRadius(5.0)), "DC 5\r\n");
    }

    #[test]
    fn write_arc_segment_data() {
        assert_eq!(
            write_record(Record::ArcSegmentData {
                radius: 10.0,
                angle_start: 270.0,
                angle_end: 290.0,
            }),
            "DA 10, 270, 290\r\n"
        );
    }

    #[test]
    fn write_arc_data() {
        let start = Coord {
            lat: 46.86222222222222,
            lng: 9.328333333333333,
        };
        let end = Coord {
            lat: 47.0,
            lng: 9.5,
        };
        assert_eq!(
            write_record(Record::ArcData { start, end }),
            "DB 46:51:44 N 009:19:42 E, 47:00:00 N 009:30:00 E\r\n"
        );
    }

    #[test]
    fn write_empty() {
        assert_eq!(write_record(Record::Empty), "\r\n");
    }

    #[test]
    fn write_ignored_records() {
        assert_eq!(write_record(Record::Comment), "");
        assert_eq!(write_record(Record::LabelPlacement), "");
        assert_eq!(write_record(Record::Pen), "");
        assert_eq!(write_record(Record::Brush), "");
    }
}
