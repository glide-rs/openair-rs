use std::{fmt, io::Write};

/// Altitude, either ground or a certain height AMSL in feet.
#[derive(Debug, Clone, PartialEq, Eq)]
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

/// Strip a prefix from a string, case-insensitively
fn strip_prefix_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    let prefix_len = prefix.len();
    if s.is_char_boundary(prefix_len) && s[..prefix_len].eq_ignore_ascii_case(prefix) {
        return Some(&s[prefix_len..]);
    }

    None
}

/// Check if a suffix indicates AMSL (above mean sea level)
/// Empty string defaults to AMSL per README: "Altitude levels without a unit specifier will be treated as feet"
fn is_amsl_suffix(s: &str) -> bool {
    s.is_empty() || s.eq_ignore_ascii_case("amsl") || s.eq_ignore_ascii_case("msl")
}

/// Check if a suffix indicates AGL (above ground level)
fn is_agl_suffix(s: &str) -> bool {
    s.eq_ignore_ascii_case("agl") || s.eq_ignore_ascii_case("gnd") || s.eq_ignore_ascii_case("sfc")
}

impl Altitude {
    /// Writes the altitude in OpenAir format.
    pub fn write<W: Write>(&self, mut writer: W) -> std::io::Result<()> {
        match self {
            Self::Gnd => write!(writer, "GND"),
            Self::FeetAmsl(n) => write!(writer, "{n}ft AMSL"),
            Self::FeetAgl(n) => write!(writer, "{n}ft AGL"),
            Self::FlightLevel(n) => write!(writer, "FL{n}"),
            Self::Unlimited => write!(writer, "UNLIM"),
            Self::Other(s) => write!(writer, "{s}"),
        }
    }

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
        // Helper to check case-insensitive equality
        let eq = |a: &str, b| a.eq_ignore_ascii_case(b);

        // Check for ground level
        // Note: SFC = Surface. Seems to be another abbreviation for GND.
        if eq(data, "gnd") || eq(data, "sfc") || data == "0" {
            return Ok(Self::Gnd);
        }

        // Check for unlimited
        if eq(data, "unl") || eq(data, "unlim") || eq(data, "unltd") || eq(data, "unlimited") {
            return Ok(Self::Unlimited);
        }

        // Check for flight level
        if let Some(after_fl) = strip_prefix_ci(data, "fl") {
            return match after_fl.trim().parse::<u16>() {
                Ok(val) => Ok(Self::FlightLevel(val)),
                Err(_) => Ok(Self::Other(data.to_string())),
            };
        }

        // Try to parse numeric altitude
        // Find where digits end to split number from unit/reference suffix
        let pos = data
            .find(|c: char| !c.is_ascii_digit())
            .unwrap_or(data.len());

        let (number, rest) = data.split_at(pos);

        let Ok(mut val) = number.parse::<i32>() else {
            return Ok(Self::Other(data.to_string()));
        };

        let rest = rest.trim();

        // Check for simple single-word patterns first (e.g., "1000 MSL", "1000 AGL")
        if is_amsl_suffix(rest) {
            return Ok(Self::FeetAmsl(val));
        }
        if is_agl_suffix(rest) {
            return Ok(Self::FeetAgl(val));
        }

        // Parse as a "unit [reference]" pattern (e.g., "ft AMSL", "m AGL", "ft", "m")
        // Split on first whitespace to separate unit from optional reference level
        let space_pos = rest.find(char::is_whitespace).unwrap_or(rest.len());
        let (unit, reference) = rest.split_at(space_pos);
        let reference = reference.trim();

        // Convert meters to feet or ensure the unit is "ft"
        if eq(unit, "m") {
            val = Self::m2ft(val)?;
        } else if !eq(unit, "ft") {
            // Unknown unit - can't parse
            return Ok(Self::Other(data.to_string()));
        }

        // Now check the reference level (or empty for AMSL default)
        if is_amsl_suffix(reference) {
            return Ok(Self::FeetAmsl(val));
        }
        if is_agl_suffix(reference) {
            return Ok(Self::FeetAgl(val));
        }

        // Unknown reference level
        Ok(Self::Other(data.to_string()))
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

    #[test]
    fn parse_msl_amsl_equivalence() {
        // MSL and AMSL should be treated identically
        assert_eq!(
            Altitude::parse("1000 MSL").unwrap(),
            Altitude::FeetAmsl(1000)
        );
        assert_eq!(
            Altitude::parse("1000 AMSL").unwrap(),
            Altitude::FeetAmsl(1000)
        );
        assert_eq!(
            Altitude::parse("1000msl").unwrap(),
            Altitude::FeetAmsl(1000)
        );
        assert_eq!(
            Altitude::parse("1000ft MSL").unwrap(),
            Altitude::FeetAmsl(1000)
        );
        assert_eq!(
            Altitude::parse("1000 ft AMSL").unwrap(),
            Altitude::FeetAmsl(1000)
        );
    }

    #[test]
    fn parse_meters_amsl() {
        // Meters should be converted to feet
        assert_eq!(Altitude::parse("100m").unwrap(), Altitude::FeetAmsl(328));
        assert_eq!(Altitude::parse("100 m").unwrap(), Altitude::FeetAmsl(328));
        assert_eq!(
            Altitude::parse("100m MSL").unwrap(),
            Altitude::FeetAmsl(328)
        );
        assert_eq!(
            Altitude::parse("100 m AMSL").unwrap(),
            Altitude::FeetAmsl(328)
        );
    }

    #[test]
    fn parse_meters_agl() {
        // Meters AGL should be converted to feet
        assert_eq!(Altitude::parse("100m agl").unwrap(), Altitude::FeetAgl(328));
        assert_eq!(
            Altitude::parse("100 m AGL").unwrap(),
            Altitude::FeetAgl(328)
        );
        assert_eq!(Altitude::parse("100m gnd").unwrap(), Altitude::FeetAgl(328));
        assert_eq!(
            Altitude::parse("100 m SFC").unwrap(),
            Altitude::FeetAgl(328)
        );
    }

    #[test]
    fn parse_whitespace_variations() {
        // No space
        assert_eq!(Altitude::parse("1000ft").unwrap(), Altitude::FeetAmsl(1000));
        assert_eq!(Altitude::parse("100m").unwrap(), Altitude::FeetAmsl(328));

        // Single space
        assert_eq!(
            Altitude::parse("1000 ft").unwrap(),
            Altitude::FeetAmsl(1000)
        );
        assert_eq!(Altitude::parse("100 m").unwrap(), Altitude::FeetAmsl(328));

        // Multiple spaces
        assert_eq!(
            Altitude::parse("1000  ft").unwrap(),
            Altitude::FeetAmsl(1000)
        );
        assert_eq!(
            Altitude::parse("1000   ft   AMSL").unwrap(),
            Altitude::FeetAmsl(1000)
        );

        // Trailing spaces (from German data)
        assert_eq!(
            Altitude::parse("1000ft MSL ").unwrap(),
            Altitude::FeetAmsl(1000)
        );
    }

    #[test]
    fn parse_case_variations() {
        // Mixed case for all keywords
        assert_eq!(Altitude::parse("1000FT").unwrap(), Altitude::FeetAmsl(1000));
        assert_eq!(Altitude::parse("1000Ft").unwrap(), Altitude::FeetAmsl(1000));
        assert_eq!(Altitude::parse("1000fT").unwrap(), Altitude::FeetAmsl(1000));
        assert_eq!(Altitude::parse("100M").unwrap(), Altitude::FeetAmsl(328));
        assert_eq!(
            Altitude::parse("1000 Msl").unwrap(),
            Altitude::FeetAmsl(1000)
        );
        assert_eq!(
            Altitude::parse("1000 aMsL").unwrap(),
            Altitude::FeetAmsl(1000)
        );
        assert_eq!(
            Altitude::parse("1000 AgL").unwrap(),
            Altitude::FeetAgl(1000)
        );
    }

    #[test]
    fn parse_unparseable() {
        // Truly unparseable inputs should become Other
        assert_eq!(
            Altitude::parse("something random").unwrap(),
            Altitude::Other("something random".to_string())
        );
        assert_eq!(
            Altitude::parse("1000xyz").unwrap(),
            Altitude::Other("1000xyz".to_string())
        );
    }

    fn write_altitude(altitude: &Altitude) -> String {
        let mut buf = Vec::new();
        altitude.write(&mut buf).unwrap();
        String::from_utf8(buf).unwrap()
    }

    #[test]
    fn write() {
        assert_eq!(write_altitude(&Altitude::Gnd), "GND");
        assert_eq!(write_altitude(&Altitude::FeetAmsl(5000)), "5000ft AMSL");
        assert_eq!(write_altitude(&Altitude::FeetAmsl(-200)), "-200ft AMSL");
        assert_eq!(write_altitude(&Altitude::FeetAgl(1500)), "1500ft AGL");
        assert_eq!(write_altitude(&Altitude::FlightLevel(195)), "FL195");
        assert_eq!(write_altitude(&Altitude::Unlimited), "UNLIM");
        assert_eq!(
            write_altitude(&Altitude::Other("custom".to_string())),
            "custom"
        );
    }
}
