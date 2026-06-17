//! The display-facing weather model, decoupled from the Open-Meteo JSON shape.

/// A weather condition, derived from a WMO weather interpretation code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Condition {
    Clear,
    MainlyClear,
    PartlyCloudy,
    Overcast,
    Fog,
    Drizzle,
    Rain,
    FreezingRain,
    Snow,
    Showers,
    SnowShowers,
    Thunderstorm,
    Unknown,
}

impl Condition {
    /// Map a WMO weather code (as returned by Open-Meteo) to a condition.
    pub fn from_wmo(code: u8) -> Self {
        match code {
            0 => Condition::Clear,
            1 => Condition::MainlyClear,
            2 => Condition::PartlyCloudy,
            3 => Condition::Overcast,
            45 | 48 => Condition::Fog,
            51 | 53 | 55 => Condition::Drizzle,
            56 | 57 | 66 | 67 => Condition::FreezingRain,
            61 | 63 | 65 => Condition::Rain,
            71 | 73 | 75 | 77 => Condition::Snow,
            80 | 81 | 82 => Condition::Showers,
            85 | 86 => Condition::SnowShowers,
            95 | 96 | 99 => Condition::Thunderstorm,
            _ => Condition::Unknown,
        }
    }

    /// Short human-readable label for the header.
    pub fn label(self) -> &'static str {
        match self {
            Condition::Clear => "Clear",
            Condition::MainlyClear => "Mainly clear",
            Condition::PartlyCloudy => "Partly cloudy",
            Condition::Overcast => "Overcast",
            Condition::Fog => "Fog",
            Condition::Drizzle => "Drizzle",
            Condition::Rain => "Rain",
            Condition::FreezingRain => "Freezing rain",
            Condition::Snow => "Snow",
            Condition::Showers => "Showers",
            Condition::SnowShowers => "Snow showers",
            Condition::Thunderstorm => "Thunderstorm",
            Condition::Unknown => "—",
        }
    }
}

/// A calendar date, parsed from an ISO `YYYY-MM-DD` string.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Date {
    pub year: i32,
    pub month: u32,
    pub day: u32,
}

impl Date {
    /// Parse `YYYY-MM-DD`; the leading date portion of an ISO timestamp is fine.
    pub fn parse(s: &str) -> Option<Self> {
        let date_part = s.split(['T', ' ']).next()?;
        let mut parts = date_part.split('-');
        let year = parts.next()?.parse().ok()?;
        let month = parts.next()?.parse().ok()?;
        let day = parts.next()?.parse().ok()?;
        if (1..=12).contains(&month) && (1..=31).contains(&day) {
            Some(Date { year, month, day })
        } else {
            None
        }
    }

    /// Day of week via Sakamoto's algorithm. 0 = Sunday … 6 = Saturday.
    fn weekday_index(self) -> usize {
        const T: [i32; 12] = [0, 3, 2, 5, 0, 3, 5, 1, 4, 6, 2, 4];
        let mut y = self.year;
        if self.month < 3 {
            y -= 1;
        }
        let m = (self.month - 1) as usize;
        (((y + y / 4 - y / 100 + y / 400 + T[m] + self.day as i32) % 7 + 7) % 7) as usize
    }

    pub fn weekday_short(self) -> &'static str {
        const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
        DAYS[self.weekday_index()]
    }

    pub fn month_short(self) -> &'static str {
        const MONTHS: [&str; 12] = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        MONTHS[(self.month - 1) as usize]
    }
}

#[derive(Debug, Clone)]
pub struct Current {
    pub temperature: f64,
    pub condition: Condition,
}

#[derive(Debug, Clone)]
pub struct Day {
    pub date: Date,
    pub temp_max: f64,
    pub temp_min: f64,
    /// Maximum precipitation probability for the day, in percent.
    pub precip_prob: Option<u8>,
    pub condition: Condition,
}

#[derive(Debug, Clone)]
pub struct Forecast {
    /// Resolved place name from geocoding (e.g. "Zagreb, Croatia").
    pub location_name: String,
    pub current: Current,
    pub days: Vec<Day>,
    /// Unit symbol shown next to temperatures, e.g. "°C".
    pub temperature_symbol: &'static str,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_iso_date_and_weekday() {
        let d = Date::parse("2026-06-17").unwrap();
        assert_eq!((d.year, d.month, d.day), (2026, 6, 17));
        // 2026-06-17 is a Wednesday.
        assert_eq!(d.weekday_short(), "Wed");
        assert_eq!(d.month_short(), "Jun");
    }

    #[test]
    fn parses_date_from_timestamp() {
        let d = Date::parse("2026-12-25T13:00").unwrap();
        assert_eq!(d.weekday_short(), "Fri");
    }

    #[test]
    fn maps_wmo_codes() {
        assert_eq!(Condition::from_wmo(0), Condition::Clear);
        assert_eq!(Condition::from_wmo(2), Condition::PartlyCloudy);
        assert_eq!(Condition::from_wmo(65), Condition::Rain);
        assert_eq!(Condition::from_wmo(95), Condition::Thunderstorm);
        assert_eq!(Condition::from_wmo(200), Condition::Unknown);
    }
}
