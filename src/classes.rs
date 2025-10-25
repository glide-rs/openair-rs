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

    /// Returns the OpenAir string representation for this class.
    pub fn to_str(&self) -> &str {
        match self {
            Self::A => "A",
            Self::B => "B",
            Self::C => "C",
            Self::D => "D",
            Self::E => "E",
            Self::F => "F",
            Self::G => "G",
            Self::Ctr => "CTR",
            Self::Restricted => "R",
            Self::Danger => "Q",
            Self::Prohibited => "P",
            Self::GliderProhibited => "GP",
            Self::WaveWindow => "W",
            Self::RadioMandatoryZone => "RMZ",
            Self::TransponderMandatoryZone => "TMZ",
            Self::Unclassified => "UNC",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_str() {
        assert_eq!(Class::A.to_str(), "A");
        assert_eq!(Class::B.to_str(), "B");
        assert_eq!(Class::C.to_str(), "C");
        assert_eq!(Class::D.to_str(), "D");
        assert_eq!(Class::E.to_str(), "E");
        assert_eq!(Class::F.to_str(), "F");
        assert_eq!(Class::G.to_str(), "G");
        assert_eq!(Class::Ctr.to_str(), "CTR");
        assert_eq!(Class::Restricted.to_str(), "R");
        assert_eq!(Class::Danger.to_str(), "Q");
        assert_eq!(Class::Prohibited.to_str(), "P");
        assert_eq!(Class::GliderProhibited.to_str(), "GP");
        assert_eq!(Class::WaveWindow.to_str(), "W");
        assert_eq!(Class::RadioMandatoryZone.to_str(), "RMZ");
        assert_eq!(Class::TransponderMandatoryZone.to_str(), "TMZ");
        assert_eq!(Class::Unclassified.to_str(), "UNC");
    }
}
