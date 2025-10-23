use std::str::FromStr;

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct ActivationTimes {
    start: Option<iso8601::DateTime>,
    end: Option<iso8601::DateTime>,
}

impl ActivationTimes {
    pub fn new(start: Option<iso8601::DateTime>, end: Option<iso8601::DateTime>) -> Self {
        Self { start, end }
    }

    pub fn none() -> Self {
        Self::new(None, None)
    }
}

impl FromStr for ActivationTimes {
    type Err = String;

    fn from_str(data: &str) -> Result<Self, Self::Err> {
        if data.eq_ignore_ascii_case("NONE") {
            return Ok(ActivationTimes::none());
        }

        let Some((start, end)) = data.split_once('/') else {
            return Err(format!("Invalid activation times record: {}", data));
        };

        let start = if start.eq_ignore_ascii_case("NONE") {
            None
        } else {
            Some(iso8601::datetime(start)?)
        };

        let end = if end.eq_ignore_ascii_case("NONE") {
            None
        } else {
            Some(iso8601::datetime(end)?)
        };

        Ok(ActivationTimes::new(start, end))
    }
}

#[cfg(test)]
mod tests {
    use insta::assert_debug_snapshot;

    use super::*;

    #[test]
    fn test_parse() {
        // Active from 12:00 to 13:00 UTC
        assert_debug_snapshot!("2023-12-16T12:00Z/2023-12-16T13:00Z".parse::<ActivationTimes>().unwrap(), @r"
        ActivationTimes {
            start: Some(
                DateTime {
                    date: YMD {
                        year: 2023,
                        month: 12,
                        day: 16,
                    },
                    time: Time {
                        hour: 12,
                        minute: 0,
                        second: 0,
                        millisecond: 0,
                        tz_offset_hours: 0,
                        tz_offset_minutes: 0,
                    },
                },
            ),
            end: Some(
                DateTime {
                    date: YMD {
                        year: 2023,
                        month: 12,
                        day: 16,
                    },
                    time: Time {
                        hour: 13,
                        minute: 0,
                        second: 0,
                        millisecond: 0,
                        tz_offset_hours: 0,
                        tz_offset_minutes: 0,
                    },
                },
            ),
        }
        ");
        // Active for the entire UTC day
        assert_debug_snapshot!("2024-12-17T00:00Z/2024-12-17T24:00Z".parse::<ActivationTimes>().unwrap(), @r"
        ActivationTimes {
            start: Some(
                DateTime {
                    date: YMD {
                        year: 2024,
                        month: 12,
                        day: 17,
                    },
                    time: Time {
                        hour: 0,
                        minute: 0,
                        second: 0,
                        millisecond: 0,
                        tz_offset_hours: 0,
                        tz_offset_minutes: 0,
                    },
                },
            ),
            end: Some(
                DateTime {
                    date: YMD {
                        year: 2024,
                        month: 12,
                        day: 17,
                    },
                    time: Time {
                        hour: 24,
                        minute: 0,
                        second: 0,
                        millisecond: 0,
                        tz_offset_hours: 0,
                        tz_offset_minutes: 0,
                    },
                },
            ),
        }
        ");
        // Active from midnight UTC until unspecified end
        assert_debug_snapshot!("2024-12-17T00:00Z/NONE".parse::<ActivationTimes>().unwrap(), @r"
        ActivationTimes {
            start: Some(
                DateTime {
                    date: YMD {
                        year: 2024,
                        month: 12,
                        day: 17,
                    },
                    time: Time {
                        hour: 0,
                        minute: 0,
                        second: 0,
                        millisecond: 0,
                        tz_offset_hours: 0,
                        tz_offset_minutes: 0,
                    },
                },
            ),
            end: None,
        }
        ");
        // Active until midnight UTC, with unknown start
        assert_debug_snapshot!("NONE/2024-12-18T00:00Z".parse::<ActivationTimes>().unwrap(), @r"
        ActivationTimes {
            start: None,
            end: Some(
                DateTime {
                    date: YMD {
                        year: 2024,
                        month: 12,
                        day: 18,
                    },
                    time: Time {
                        hour: 0,
                        minute: 0,
                        second: 0,
                        millisecond: 0,
                        tz_offset_hours: 0,
                        tz_offset_minutes: 0,
                    },
                },
            ),
        }
        ");
        // No defined time - inactive
        assert_debug_snapshot!("NONE".parse::<ActivationTimes>().unwrap(), @r"
        ActivationTimes {
            start: None,
            end: None,
        }
        ");
    }
}
