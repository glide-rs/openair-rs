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

/// An arc (DB record).
#[derive(Debug, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Arc {
    pub centerpoint: Coord,
    pub start: Coord,
    pub end: Coord,
    pub direction: Direction,
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
