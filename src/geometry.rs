use std::fmt;

use crate::Coord;

/// Arc direction, either clockwise or counterclockwise.
#[derive(Debug, PartialEq, Eq, Copy, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
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
    pub fn parse(data: &str) -> Result<Self, String> {
        match data {
            "+" => Ok(Self::Cw),
            "-" => Ok(Self::Ccw),
            _ => Err(format!("Invalid direction: {}", data)),
        }
    }
}

/// An arc segment (DA record).
#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
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

    pub fn parse(data: &str, centerpoint: Coord, direction: Direction) -> Result<Self, String> {
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
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Arc {
    pub centerpoint: Coord,
    pub start: Coord,
    pub end: Coord,
    pub direction: Direction,
}

impl Arc {
    pub fn parse(data: &str, centerpoint: Coord, direction: Direction) -> Result<Self, String> {
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
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[cfg_attr(feature = "serde", serde(tag = "type"))]
pub enum PolygonSegment {
    Point(Coord),
    Arc(Arc),
    ArcSegment(ArcSegment),
}

#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
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

#[cfg(test)]
mod tests {
    use insta::{assert_compact_debug_snapshot, assert_debug_snapshot};

    use super::*;

    static COORD: Coord = Coord { lat: 1.0, lng: 2.0 };

    #[test]
    fn parse_ok() {
        assert_debug_snapshot!(ArcSegment::parse("10,270,290", COORD.clone(), Direction::Cw).unwrap(), @r"
        ArcSegment {
            centerpoint: Coord {
                lat: 1.0,
                lng: 2.0,
            },
            radius: 10.0,
            angle_start: 270.0,
            angle_end: 290.0,
            direction: Cw,
        }
        ");

        assert_debug_snapshot!(ArcSegment::parse("23,0,30", COORD.clone(), Direction::Ccw).unwrap(), @r"
        ArcSegment {
            centerpoint: Coord {
                lat: 1.0,
                lng: 2.0,
            },
            radius: 23.0,
            angle_start: 0.0,
            angle_end: 30.0,
            direction: Ccw,
        }
        ");
    }

    #[test]
    fn parse_with_spaces() {
        assert_debug_snapshot!(ArcSegment::parse(" 10 ,    270 ,290", COORD.clone(), Direction::Cw).unwrap(), @r"
        ArcSegment {
            centerpoint: Coord {
                lat: 1.0,
                lng: 2.0,
            },
            radius: 10.0,
            angle_start: 270.0,
            angle_end: 290.0,
            direction: Cw,
        }
        ");
    }

    #[test]
    fn parse_invalid_too_many() {
        assert_compact_debug_snapshot!(
            ArcSegment::parse(" 10 ,    270 ,290,", COORD.clone(), Direction::Cw),
            @r#"Err("Invalid arc segment data:  10 ,    270 ,290,")"#,
        );
    }

    #[test]
    fn parse_invalid_angle_too_large() {
        assert_compact_debug_snapshot!(
            ArcSegment::parse("10,270,361", COORD.clone(), Direction::Cw),
            @r#"Err("Angle 361 too large")"#,
        );
    }

    #[test]
    fn parse_invalid_angle_negative() {
        assert_compact_debug_snapshot!(
            ArcSegment::parse("10,270,-10", COORD.clone(), Direction::Cw),
            @r#"Err("Angle -10 is negative")"#,
        );
    }
}
