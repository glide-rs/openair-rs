use std::fmt;

/// Airspace class.
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
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
        write!(f, "{:?}", self)
    }
}

impl Class {
    pub fn parse(data: &str) -> Result<Self, String> {
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
