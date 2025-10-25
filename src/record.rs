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
}
