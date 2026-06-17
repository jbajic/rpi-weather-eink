//! The display-facing weather model, decoupled from the Open-Meteo JSON shape.

use crate::config::Language;

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
    pub fn label(self, lang: Language) -> &'static str {
        match lang {
            Language::En => match self {
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
            },
            Language::Hr => match self {
                Condition::Clear => "Vedro",
                Condition::MainlyClear => "Pretezno vedro",
                Condition::PartlyCloudy => "Djelomicno oblacno",
                Condition::Overcast => "Oblacno",
                Condition::Fog => "Magla",
                Condition::Drizzle => "Rosulja",
                Condition::Rain => "Kisa",
                Condition::FreezingRain => "Ledena kisa",
                Condition::Snow => "Snijeg",
                Condition::Showers => "Pljuskovi",
                Condition::SnowShowers => "Snjezni pljuskovi",
                Condition::Thunderstorm => "Grmljavina",
                Condition::Unknown => "—",
            },
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

    pub fn weekday_short(self, lang: Language) -> &'static str {
        const EN: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
        const HR: [&str; 7] = ["Ned", "Pon", "Uto", "Sri", "Cet", "Pet", "Sub"];
        let days = match lang {
            Language::En => &EN,
            Language::Hr => &HR,
        };
        days[self.weekday_index()]
    }

    pub fn month_short(self, lang: Language) -> &'static str {
        const EN: [&str; 12] = [
            "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
        ];
        const HR: [&str; 12] = [
            "Sij", "Velj", "Ozu", "Tra", "Svi", "Lip", "Srp", "Kol", "Ruj", "Lis", "Stu", "Pro",
        ];
        let months = match lang {
            Language::En => &EN,
            Language::Hr => &HR,
        };
        months[(self.month - 1) as usize]
    }

    /// Civil date from a count of days since the Unix epoch (Hinnant's algorithm).
    fn from_days_since_epoch(days: i64) -> Date {
        let z = days + 719_468;
        let era = z.div_euclid(146_097);
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let day = (doy - (153 * mp + 2) / 5 + 1) as u32;
        let month = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
        let year = (yoe + era * 400) as i32 + i32::from(month <= 2);
        Date { year, month, day }
    }
}

/// Format a local Unix timestamp (seconds, already offset to local time) as
/// `DD.MM HH:MM`, e.g. "17.06 22:05".
pub fn format_local_timestamp(local_unix_secs: i64) -> String {
    let days = local_unix_secs.div_euclid(86_400);
    let secs = local_unix_secs.rem_euclid(86_400);
    let hour = secs / 3600;
    let minute = (secs % 3600) / 60;
    let date = Date::from_days_since_epoch(days);
    format!("{:02}.{:02} {hour:02}:{minute:02}", date.day, date.month)
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
    /// Local wall-clock time the forecast was fetched/rendered, e.g. "Wed 17 Jun 22:05".
    pub refreshed_at: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_iso_date_and_weekday() {
        let d = Date::parse("2026-06-17").unwrap();
        assert_eq!((d.year, d.month, d.day), (2026, 6, 17));
        // 2026-06-17 is a Wednesday.
        assert_eq!(d.weekday_short(Language::En), "Wed");
        assert_eq!(d.weekday_short(Language::Hr), "Sri");
        assert_eq!(d.month_short(Language::En), "Jun");
        assert_eq!(d.month_short(Language::Hr), "Lip");
    }

    #[test]
    fn parses_date_from_timestamp() {
        let d = Date::parse("2026-12-25T13:00").unwrap();
        assert_eq!(d.weekday_short(Language::En), "Fri");
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
