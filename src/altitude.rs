use std::{fmt, sync::LazyLock};

use regex::Regex;

/// Altitude, either ground or a certain height AMSL in feet.
#[derive(Debug, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
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

    pub fn parse(data: &str) -> Result<Self, String> {
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
                    Err(_) => Err(format!("Invalid altitude: {}", fl)),
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

#[cfg(test)]
mod tests {
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
