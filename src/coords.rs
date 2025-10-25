/// A coordinate pair (WGS84).
#[derive(Debug, PartialEq, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Coord {
    pub lat: f64,
    pub lng: f64,
}

impl Coord {
    pub fn parse(data: &str) -> Result<Self, String> {
        let input = data.trim();
        let err = || format!("Invalid coord: \"{data}\"");

        // Parse latitude coordinate and direction
        let (mut lat, rest) = parse_coord_component(input, true).map_err(|_| err())?;
        let (lat_is_negative, rest) = parse_direction(rest, true).map_err(|_| err())?;
        if lat_is_negative {
            lat = -lat;
        }

        // Skip whitespace and optional comma
        let rest = rest.trim_start();
        let rest = rest.strip_prefix(',').unwrap_or(rest).trim_start();

        // Parse longitude coordinate and direction
        let (mut lng, rest) = parse_coord_component(rest, false).map_err(|_| err())?;
        let (lng_is_negative, _rest) = parse_direction(rest, false).map_err(|_| err())?;
        if lng_is_negative {
            lng = -lng;
        }

        Ok(Self { lat, lng })
    }
}

fn parse_coord_component(input: &str, is_lat: bool) -> Result<(f64, &str), ()> {
    // Parse degrees
    let pos = input.find(|c: char| !c.is_ascii_digit()).ok_or(())?;

    let max_digits = if is_lat { 2 } else { 3 };
    if pos > max_digits {
        return Err(());
    }

    let (deg_str, rest) = input.split_at(pos);
    let degrees = f64::from(deg_str.parse::<u8>().map_err(|_| ())?);

    // Validate degree ranges
    if (is_lat && degrees > 90.) || (!is_lat && degrees > 180.) {
        return Err(());
    }

    // Expect colon
    let rest = rest.strip_prefix(':').ok_or(())?;

    // Parse minutes
    let pos = rest.find(|c: char| !c.is_ascii_digit()).ok_or(())?;
    if pos > 2 {
        return Err(());
    }
    let (min_str, rest) = rest.split_at(pos);
    let minutes = f64::from(min_str.parse::<u8>().map_err(|_| ())?);

    // Log warning for invalid minutes
    if minutes >= 60. {
        log::debug!("Minutes >= 60 in coordinate: {}", input);
    }

    // Check if this is DDM format (decimal minutes)
    if rest.starts_with('.') {
        // DDM format: parse fractional minutes (e.g., ".44" -> 0.44)
        let pos = rest
            .find(|c: char| !c.is_ascii_digit() && c != '.')
            .unwrap_or(rest.len());
        let (frac_str, rest) = rest.split_at(pos);
        let frac_minutes = frac_str.parse::<f64>().map_err(|_| ())?;

        // Calculate decimal degrees for DDM format
        let total = degrees + (minutes + frac_minutes) / 60.0;

        return Ok((total, rest));
    }

    // DMS format: expect colon then parse seconds (with optional fractional part)
    let rest = rest.strip_prefix(':').ok_or(())?;

    // Find end of integer part of seconds
    let int_pos = rest
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(rest.len());

    // Check that integer part has at most 2 digits
    if int_pos > 2 {
        return Err(());
    }

    // Find end of seconds (including fractional part)
    let pos = rest
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(rest.len());

    let (sec_str, rest) = rest.split_at(pos);
    let seconds = sec_str.parse::<f64>().map_err(|_| ())?;

    // Log warning for invalid seconds (check integer part)
    if seconds >= 60. {
        log::debug!("Seconds >= 60 in coordinate: {}", input);
    }

    // Calculate decimal degrees for DMS format
    let total = degrees + minutes / 60.0 + seconds / 3600.0;

    Ok((total, rest))
}

fn parse_direction(input: &str, is_lat: bool) -> Result<(bool, &str), ()> {
    let input = input.trim_start();
    let ch = input.chars().next().ok_or(())?;

    let is_negative = if is_lat {
        match ch {
            'N' | 'n' => false,
            'S' | 's' => true,
            _ => return Err(()),
        }
    } else {
        match ch {
            'E' | 'e' => false,
            'W' | 'w' => true,
            _ => return Err(()),
        }
    };

    Ok((is_negative, &input[ch.len_utf8()..]))
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
    fn parse_boundary_validation() {
        // Latitude degree boundaries
        assert_compact_debug_snapshot!(Coord::parse("90:00:00 N 000:00:00 E"), @"Ok(Coord { lat: 90.0, lng: 0.0 })");
        assert_compact_debug_snapshot!(Coord::parse("91:00:00 N 000:00:00 E"), @r#"Err("Invalid coord: \"91:00:00 N 000:00:00 E\"")"#);

        // Longitude degree boundaries
        assert_compact_debug_snapshot!(Coord::parse("00:00:00 N 180:00:00 E"), @"Ok(Coord { lat: 0.0, lng: 180.0 })");
        assert_compact_debug_snapshot!(Coord::parse("00:00:00 N 181:00:00 E"), @r#"Err("Invalid coord: \"00:00:00 N 181:00:00 E\"")"#);

        // Single-digit latitude degrees
        assert_compact_debug_snapshot!(Coord::parse("5:00:00 N 000:00:00 E"), @"Ok(Coord { lat: 5.0, lng: 0.0 })");
    }

    #[test]
    fn parse_invalid_minutes_seconds() {
        // Minutes >= 60 should parse but log warning
        assert_compact_debug_snapshot!(Coord::parse("42:60:00 N 001:00:00 E"), @"Ok(Coord { lat: 43.0, lng: 1.0 })");

        // Seconds >= 60 should parse but log warning
        assert_compact_debug_snapshot!(Coord::parse("42:00:60 N 001:00:00 E"), @"Ok(Coord { lat: 42.016666666666666, lng: 1.0 })");
    }

    #[test]
    fn parse_digit_count_limits() {
        // 3-digit latitude degrees should fail
        assert_compact_debug_snapshot!(Coord::parse("123:00:00 N 000:00:00 E"), @r#"Err("Invalid coord: \"123:00:00 N 000:00:00 E\"")"#);

        // 3-digit minutes should fail
        assert_compact_debug_snapshot!(Coord::parse("45:123:00 N 000:00:00 E"), @r#"Err("Invalid coord: \"45:123:00 N 000:00:00 E\"")"#);

        // 3-digit seconds should fail
        assert_compact_debug_snapshot!(Coord::parse("45:00:123 N 000:00:00 E"), @r#"Err("Invalid coord: \"45:00:123 N 000:00:00 E\"")"#);
    }

    #[test]
    fn parse_invalid() {
        assert_compact_debug_snapshot!(Coord::parse("46:51:44 Q 009:19:42 R"), @r#"Err("Invalid coord: \"46:51:44 Q 009:19:42 R\"")"#);
        assert_compact_debug_snapshot!(Coord::parse("46x51x44 S 009x19x42 W"), @r#"Err("Invalid coord: \"46x51x44 S 009x19x42 W\"")"#);
    }
}
