use std::sync::LazyLock;

use regex::Regex;

/// A coordinate pair (WGS84).
#[derive(Debug, PartialEq, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Coord {
    pub lat: f64,
    pub lng: f64,
}

impl Coord {
    fn parse_number_opt(val: Option<&str>) -> Result<u16, ()> {
        val.and_then(|v| v.parse::<u16>().ok()).ok_or(())
    }

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

    pub fn parse(data: &str) -> Result<Self, String> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
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
