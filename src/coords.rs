use std::{io::Write, sync::LazyLock};

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

    /// Writes coordinate in OpenAir DMS format.
    ///
    /// Format: `DD:MM:SS N/S DDD:MM:SS E/W`
    pub fn write<W: Write>(&self, mut writer: W) -> std::io::Result<()> {
        let total = (self.lat.abs() * 3600.0).round();
        let lat_deg = (total / 3600.0).trunc() as u16;
        let remaining = total - f64::from(lat_deg) * 3600.0;
        let lat_min = (remaining / 60.0).trunc() as u16;
        let lat_sec = (remaining - f64::from(lat_min) * 60.0) as u16;
        let lat_dir = if self.lat >= 0.0 { 'N' } else { 'S' };

        let total = (self.lng.abs() * 3600.0).round();
        let lng_deg = (total / 3600.0).trunc() as u16;
        let remaining = total - f64::from(lng_deg) * 3600.0;
        let lng_min = (remaining / 60.0).trunc() as u16;
        let lng_sec = (remaining - f64::from(lng_min) * 60.0) as u16;
        let lng_dir = if self.lng >= 0.0 { 'E' } else { 'W' };

        write!(
            writer,
            "{lat_deg:02}:{lat_min:02}:{lat_sec:02} {lat_dir} {lng_deg:03}:{lng_min:02}:{lng_sec:02} {lng_dir}",
        )
    }
}

#[cfg(test)]
mod tests {
    use insta::assert_compact_debug_snapshot;

    use super::*;

    #[test]
    fn parse_valid() {
        // With spaces
        assert_compact_debug_snapshot!(Coord::parse("46:51:44 N 009:19:42 E"), @"Ok(Coord { lat: 46.86222222222222, lng: 9.328333333333333 })");

        // Without spaces
        assert_compact_debug_snapshot!(Coord::parse("46:51:44N 009:19:42E"), @"Ok(Coord { lat: 46.86222222222222, lng: 9.328333333333333 })");

        // Dot between min and sec
        assert_compact_debug_snapshot!(Coord::parse("46:51.44 N 009:19.42 E"), @"Ok(Coord { lat: 46.85733333333334, lng: 9.323666666666666 })");

        // South / west
        assert_compact_debug_snapshot!(Coord::parse("46:51:44 S 009:19:42 W"), @"Ok(Coord { lat: -46.86222222222222, lng: -9.328333333333333 })");

        // Fractional part
        assert_compact_debug_snapshot!(Coord::parse("1:0:0.123 N 2:0:1.2 E"), @"Ok(Coord { lat: 1.0000341666666666, lng: 2.0003333333333333 })");

        // Comma in between
        assert_compact_debug_snapshot!(Coord::parse("45:42:21 N, 000:38:41 W"), @"Ok(Coord { lat: 45.70583333333334, lng: -0.6447222222222222 })");

        // Lowercase letters
        assert_compact_debug_snapshot!(Coord::parse("49:33:8 n 5:47:37 e"), @"Ok(Coord { lat: 49.55222222222222, lng: 5.793611111111111 })");
    }

    #[test]
    fn parse_invalid() {
        assert_compact_debug_snapshot!(Coord::parse("46:51:44 Q 009:19:42 R"), @r#"Err("Invalid coord: \"46:51:44 Q 009:19:42 R\"")"#);
        assert_compact_debug_snapshot!(Coord::parse("46x51x44 S 009x19x42 W"), @r#"Err("Invalid coord: \"46x51x44 S 009x19x42 W\"")"#);
    }

    fn lat_lng(lat: f64, lng: f64) -> Coord {
        Coord { lat, lng }
    }

    fn write_coord(coord: &Coord) -> String {
        let mut buf = Vec::new();
        coord.write(&mut buf).unwrap();
        String::from_utf8(buf).unwrap()
    }

    #[test]
    fn write_valid() {
        // Zero
        assert_compact_debug_snapshot!(write_coord(&lat_lng(0.0, 0.0)), @r#""00:00:00 N 000:00:00 E""#);

        // All four quadrants
        assert_compact_debug_snapshot!(
            write_coord(&lat_lng(46.86222222222222, 9.328333333333333)),
            @r#""46:51:44 N 009:19:42 E""#
        );
        assert_compact_debug_snapshot!(
            write_coord(&lat_lng(-46.86222222222222, -9.328333333333333)),
            @r#""46:51:44 S 009:19:42 W""#
        );
        assert_compact_debug_snapshot!(
            write_coord(&lat_lng(45.70583333333334, -0.6447222222222222)),
            @r#""45:42:21 N 000:38:41 W""#
        );
        assert_compact_debug_snapshot!(
            write_coord(&lat_lng(-49.55222222222222, 5.793611111111111)),
            @r#""49:33:08 S 005:47:37 E""#
        );

        // 3-digit longitude degrees
        assert_compact_debug_snapshot!(
            write_coord(&lat_lng(0.0, 123.456789)),
            @r#""00:00:00 N 123:27:24 E""#
        );

        // Rounding down (< 0.5 seconds)
        assert_compact_debug_snapshot!(
            write_coord(&lat_lng(
                1.0 + 0.0 / 60.0 + 0.4 / 3600.0,
                2.0 + 0.0 / 60.0 + 0.4 / 3600.0,
            )),
            @r#""01:00:00 N 002:00:00 E""#
        );

        // Rounding up (≥ 0.5 seconds)
        assert_compact_debug_snapshot!(
            write_coord(&lat_lng(
                1.0 + 0.0 / 60.0 + 0.5 / 3600.0,
                2.0 + 0.0 / 60.0 + 0.5 / 3600.0,
            )),
            @r#""01:00:01 N 002:00:01 E""#
        );

        // Rounding causes seconds rollover to minutes
        assert_compact_debug_snapshot!(
            write_coord(&lat_lng(
                1.0 + 30.0 / 60.0 + 59.5 / 3600.0,
                2.0 + 45.0 / 60.0 + 59.5 / 3600.0,
            )),
            @r#""01:31:00 N 002:46:00 E""#
        );

        // Rounding causes minutes rollover to degrees
        assert_compact_debug_snapshot!(
            write_coord(&lat_lng(
                1.0 + 59.0 / 60.0 + 59.5 / 3600.0,
                2.0 + 59.0 / 60.0 + 59.5 / 3600.0,
            )),
            @r#""02:00:00 N 003:00:00 E""#
        );
    }
}
