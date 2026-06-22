//! met.no (Norwegian Meteorological Institute) Locationforecast client — the
//! data behind Yr.no. No API key, but a descriptive `User-Agent` is mandatory
//! or the API returns 403. Geocoding is shared (see `geocode`).
//!
//! The `complete` product is used because `compact` omits
//! `probability_of_precipitation`. Forecasts are returned as a UTC timeseries
//! (hourly near-term, 6-hourly later), which is folded into local calendar
//! days using the IANA timezone of the resolved place.

use anyhow::{Context, Result, anyhow};
use chrono::{DateTime, Datelike, Offset, TimeZone, Timelike, Utc};
use chrono_tz::Tz;
use serde::Deserialize;

use crate::config::{Config, TemperatureUnit};

use super::geocode;
use super::model::{Condition, Current, Date, Day, Forecast, format_local_timestamp};

const FORECAST_URL: &str = "https://api.met.no/weatherapi/locationforecast/2.0/complete";
/// met.no requires a unique, descriptive User-Agent with contact info.
const USER_AGENT: &str = "eink-weather-display/0.1 jure.bajic@arangodb.com";
/// Match the Open-Meteo path: today plus the next six days.
const FORECAST_DAYS: usize = 7;

pub fn fetch_forecast(config: &Config) -> Result<Forecast> {
    let agent = super::http_agent();
    let place = geocode::resolve_place(&agent, config)?;
    let tz = place
        .timezone
        .as_deref()
        .and_then(|name| name.parse::<Tz>().ok())
        .unwrap_or(chrono_tz::UTC);

    let raw = fetch_raw(&agent, place.latitude, place.longitude)?;
    let (current, days) = assemble(&raw, tz, config.units.temperature)?;

    Ok(Forecast {
        location_name: geocode::display_name(&place, config.language),
        current,
        days,
        temperature_symbol: config.units.temperature.symbol(),
        refreshed_at: format_local_timestamp(now_local_secs(tz)),
    })
}

fn fetch_raw(agent: &ureq::Agent, latitude: f64, longitude: f64) -> Result<MetResponse> {
    eprintln!("[weather] fetching met.no forecast @ {latitude:.4},{longitude:.4} ...");
    let mut response = agent
        .get(FORECAST_URL)
        .header("User-Agent", USER_AGENT)
        .query("lat", format!("{latitude:.4}"))
        .query("lon", format!("{longitude:.4}"))
        .call()
        .context("met.no forecast request failed")?;

    let parsed = response
        .body_mut()
        .read_json()
        .context("decoding met.no forecast response")?;
    eprintln!("[weather] met.no forecast response received");
    Ok(parsed)
}

/// Current wall-clock time in the location's timezone, as a local Unix second
/// count for `format_local_timestamp`.
fn now_local_secs(tz: Tz) -> i64 {
    let now = Utc::now();
    let offset = tz.offset_from_utc_datetime(&now.naive_utc()).fix();
    now.timestamp() + i64::from(offset.local_minus_utc())
}

/// Fold the UTC timeseries into current conditions and up to `FORECAST_DAYS`
/// local days. Pure (no network) so it can be unit-tested against a fixture.
fn assemble(raw: &MetResponse, tz: Tz, unit: TemperatureUnit) -> Result<(Current, Vec<Day>)> {
    let series = &raw.properties.timeseries;
    let first = series
        .first()
        .ok_or_else(|| anyhow!("met.no forecast contained no timeseries entries"))?;

    let current = Current {
        temperature: convert_temp(
            first
                .data
                .instant
                .details
                .air_temperature
                .ok_or_else(|| anyhow!("met.no entry missing current temperature"))?,
            unit,
        ),
        condition: first
            .data
            .symbol_code()
            .map(Condition::from_met_symbol)
            .unwrap_or(Condition::Unknown),
    };

    let mut acc: Vec<DayAcc> = Vec::new();
    for entry in series {
        let Ok(local) = DateTime::parse_from_rfc3339(&entry.time) else {
            continue;
        };
        let local = local.with_timezone(&tz);
        let date = Date {
            year: local.year(),
            month: local.month(),
            day: local.day(),
        };
        let day = match acc.iter_mut().find(|d| d.date == date) {
            Some(d) => d,
            None => {
                if acc.len() >= FORECAST_DAYS {
                    continue;
                }
                acc.push(DayAcc::new(date));
                acc.last_mut().expect("just pushed")
            }
        };
        day.absorb(entry, minutes_from_noon(&local));
    }

    let days: Vec<Day> = acc.into_iter().map(|d| d.finish(unit)).collect();
    if days.is_empty() {
        return Err(anyhow!("no parseable days in met.no forecast"));
    }
    Ok((current, days))
}

fn minutes_from_noon(local: &DateTime<Tz>) -> i64 {
    let minute_of_day = i64::from(local.hour() * 60 + local.minute());
    (minute_of_day - 12 * 60).abs()
}

fn convert_temp(celsius: f64, unit: TemperatureUnit) -> f64 {
    match unit {
        TemperatureUnit::Celsius => celsius,
        TemperatureUnit::Fahrenheit => celsius * 9.0 / 5.0 + 32.0,
    }
}

/// Per-day accumulator over the timeseries entries that fall on that local day.
struct DayAcc {
    date: Date,
    temp_min: Option<f64>,
    temp_max: Option<f64>,
    precip_prob: Option<f64>,
    /// Representative symbol, chosen from the entry closest to local noon.
    symbol: Option<String>,
    symbol_distance: i64,
}

impl DayAcc {
    fn new(date: Date) -> Self {
        Self {
            date,
            temp_min: None,
            temp_max: None,
            precip_prob: None,
            symbol: None,
            symbol_distance: i64::MAX,
        }
    }

    fn absorb(&mut self, entry: &Entry, distance_from_noon: i64) {
        if let Some(t) = entry.data.instant.details.air_temperature {
            push_min(&mut self.temp_min, t);
            push_max(&mut self.temp_max, t);
        }
        if let Some(details) = entry
            .data
            .next_6_hours
            .as_ref()
            .and_then(|p| p.details.as_ref())
        {
            if let Some(t) = details.air_temperature_min {
                push_min(&mut self.temp_min, t);
            }
            if let Some(t) = details.air_temperature_max {
                push_max(&mut self.temp_max, t);
            }
        }
        if let Some(p) = entry.data.precipitation_probability() {
            self.precip_prob = Some(self.precip_prob.map_or(p, |cur| cur.max(p)));
        }
        if let Some(code) = entry.data.symbol_code()
            && distance_from_noon < self.symbol_distance
        {
            self.symbol = Some(code.to_string());
            self.symbol_distance = distance_from_noon;
        }
    }

    fn finish(self, unit: TemperatureUnit) -> Day {
        Day {
            date: self.date,
            temp_max: convert_temp(self.temp_max.unwrap_or(0.0), unit),
            temp_min: convert_temp(self.temp_min.unwrap_or(0.0), unit),
            precip_prob: self.precip_prob.map(|p| p.round().clamp(0.0, 100.0) as u8),
            condition: self
                .symbol
                .as_deref()
                .map(Condition::from_met_symbol)
                .unwrap_or(Condition::Unknown),
        }
    }
}

fn push_min(slot: &mut Option<f64>, value: f64) {
    *slot = Some(slot.map_or(value, |cur| cur.min(value)));
}

fn push_max(slot: &mut Option<f64>, value: f64) {
    *slot = Some(slot.map_or(value, |cur| cur.max(value)));
}

// --- met.no Locationforecast JSON wire types ------------------------------

#[derive(Debug, Deserialize)]
struct MetResponse {
    properties: Properties,
}

#[derive(Debug, Deserialize)]
struct Properties {
    timeseries: Vec<Entry>,
}

#[derive(Debug, Deserialize)]
struct Entry {
    time: String,
    data: EntryData,
}

#[derive(Debug, Deserialize)]
struct EntryData {
    instant: Instant,
    next_1_hours: Option<Period>,
    next_6_hours: Option<Period>,
}

impl EntryData {
    /// Symbol code, preferring the finer-grained near-term block.
    fn symbol_code(&self) -> Option<&str> {
        self.next_1_hours
            .as_ref()
            .and_then(|p| p.summary.symbol_code.as_deref())
            .or_else(|| {
                self.next_6_hours
                    .as_ref()
                    .and_then(|p| p.summary.symbol_code.as_deref())
            })
    }

    fn precipitation_probability(&self) -> Option<f64> {
        let from = |p: &Option<Period>| {
            p.as_ref()
                .and_then(|p| p.details.as_ref())
                .and_then(|d| d.probability_of_precipitation)
        };
        match (from(&self.next_1_hours), from(&self.next_6_hours)) {
            (Some(a), Some(b)) => Some(a.max(b)),
            (a, b) => a.or(b),
        }
    }
}

#[derive(Debug, Deserialize)]
struct Instant {
    details: InstantDetails,
}

#[derive(Debug, Deserialize)]
struct InstantDetails {
    air_temperature: Option<f64>,
}

#[derive(Debug, Deserialize)]
struct Period {
    #[serde(default)]
    summary: Summary,
    details: Option<PeriodDetails>,
}

#[derive(Debug, Default, Deserialize)]
struct Summary {
    symbol_code: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PeriodDetails {
    air_temperature_max: Option<f64>,
    air_temperature_min: Option<f64>,
    probability_of_precipitation: Option<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"{
        "properties": {
            "timeseries": [
                {
                    "time": "2026-06-22T10:00:00Z",
                    "data": {
                        "instant": {"details": {"air_temperature": 19.0}},
                        "next_1_hours": {
                            "summary": {"symbol_code": "clearsky_day"},
                            "details": {"probability_of_precipitation": 5.0}
                        },
                        "next_6_hours": {
                            "summary": {"symbol_code": "fair_day"},
                            "details": {
                                "air_temperature_max": 26.0,
                                "air_temperature_min": 15.0,
                                "probability_of_precipitation": 10.0
                            }
                        }
                    }
                },
                {
                    "time": "2026-06-22T22:00:00Z",
                    "data": {
                        "instant": {"details": {"air_temperature": 14.0}},
                        "next_6_hours": {
                            "summary": {"symbol_code": "rain"},
                            "details": {
                                "air_temperature_max": 16.0,
                                "air_temperature_min": 12.0,
                                "probability_of_precipitation": 70.0
                            }
                        }
                    }
                },
                {
                    "time": "2026-06-23T12:00:00Z",
                    "data": {
                        "instant": {"details": {"air_temperature": 22.0}},
                        "next_6_hours": {
                            "summary": {"symbol_code": "heavyrainshowersandthunder_day"},
                            "details": {
                                "air_temperature_max": 24.0,
                                "air_temperature_min": 18.0,
                                "probability_of_precipitation": 90.0
                            }
                        }
                    }
                }
            ]
        }
    }"#;

    #[test]
    fn assembles_forecast_in_local_timezone() {
        let raw: MetResponse = serde_json::from_str(FIXTURE).unwrap();
        // Europe/Zagreb is UTC+2 in June, so 2026-06-22T22:00Z is still the 23rd
        // locally — verify the day grouping respects the local timezone.
        let tz: Tz = "Europe/Zagreb".parse().unwrap();
        let (current, days) = assemble(&raw, tz, TemperatureUnit::Celsius).unwrap();

        assert_eq!(current.temperature, 19.0);
        assert_eq!(current.condition, Condition::Clear);

        // 22:00Z -> 00:00 local on the 23rd, so it lands on day two, not day one.
        assert_eq!(days.len(), 2);
        let (jun22, jun23) = (&days[0], &days[1]);

        assert_eq!((jun22.date.month, jun22.date.day), (6, 22));
        assert_eq!(jun22.temp_max, 26.0);
        assert_eq!(jun22.temp_min, 15.0);
        assert_eq!(jun22.precip_prob, Some(10));
        // 10:00Z == 12:00 local: the noon-closest sample, whose finer-grained
        // next_1_hours symbol ("clearsky_day") wins.
        assert_eq!(jun22.condition, Condition::Clear);

        assert_eq!((jun23.date.month, jun23.date.day), (6, 23));
        assert_eq!(jun23.temp_max, 24.0);
        assert_eq!(jun23.precip_prob, Some(90));
        assert_eq!(jun23.condition, Condition::Thunderstorm);
    }

    #[test]
    fn converts_to_fahrenheit() {
        let raw: MetResponse = serde_json::from_str(FIXTURE).unwrap();
        let tz: Tz = "Europe/Zagreb".parse().unwrap();
        let (current, _) = assemble(&raw, tz, TemperatureUnit::Fahrenheit).unwrap();
        // 19 C == 66.2 F
        assert!((current.temperature - 66.2).abs() < 1e-9);
    }
}
