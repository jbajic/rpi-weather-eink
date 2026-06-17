//! Open-Meteo client: geocode a city name, then fetch a 7-day forecast.
//! No API key required.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

use crate::config::{Config, Language};

use super::model::{Condition, Current, Date, Day, Forecast, format_local_timestamp};

const GEOCODE_URL: &str = "https://geocoding-api.open-meteo.com/v1/search";
const FORECAST_URL: &str = "https://api.open-meteo.com/v1/forecast";
const FORECAST_DAYS: u32 = 7;
/// On-disk cache of the resolved coordinates (in the working directory).
const GEOCODE_CACHE_FILE: &str = "geocode_cache.json";
/// Cap each HTTP request so a flaky network can't hang the daemon; on timeout
/// the call errors and the daemon simply retries on the next refresh tick.
const HTTP_TIMEOUT: Duration = Duration::from_secs(20);

/// Resolve the configured city and fetch its weekly forecast.
pub fn fetch_forecast(config: &Config) -> Result<Forecast> {
    let agent = http_agent();
    let place = resolve_place(&agent, config)?;
    let raw = fetch_raw(&agent, config, place.latitude, place.longitude)?;

    let current = Current {
        temperature: raw.current.temperature_2m,
        condition: Condition::from_wmo(raw.current.weather_code as u8),
    };

    let days = build_days(&raw.daily)?;
    let country = localized_country(
        place.country_code.as_deref(),
        place.country,
        config.language,
    );
    let location_name = match country {
        Some(country) => format!("{}, {}", place.name, country),
        None => place.name,
    };

    Ok(Forecast {
        location_name,
        current,
        days,
        temperature_symbol: config.units.temperature.symbol(),
        refreshed_at: format_local_timestamp(now_local_secs(raw.utc_offset_seconds)),
    })
}

/// Preferred display name for a country. The geocoder's localized name can be
/// formal (e.g. "Republika Hrvatska"); override the cases we care about.
fn localized_country(
    code: Option<&str>,
    api_name: Option<String>,
    lang: Language,
) -> Option<String> {
    if lang == Language::Hr && code.is_some_and(|c| c.eq_ignore_ascii_case("HR")) {
        return Some("Hrvatska".to_string());
    }
    api_name
}

/// Current wall-clock time, shifted into the location's local timezone using
/// the offset Open-Meteo reports for the requested coordinates.
fn now_local_secs(utc_offset_seconds: i64) -> i64 {
    let now_utc = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    now_utc + utc_offset_seconds
}

fn http_agent() -> ureq::Agent {
    ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .timeout_global(Some(HTTP_TIMEOUT))
            .build(),
    )
}

/// Resolve coordinates for the configured city, reusing the on-disk cache when
/// it matches the current city/country/language; otherwise geocode and cache.
fn resolve_place(agent: &ureq::Agent, config: &Config) -> Result<GeoResult> {
    let key = GeoKey {
        city: config.location.city.clone(),
        country: config.location.country.clone(),
        language: config.language.code().to_string(),
    };

    if let Some(cached) = read_geocode_cache() {
        if cached.key == key {
            eprintln!("[weather] using cached coordinates for {:?}", key.city);
            return Ok(cached.place);
        }
    }

    let place = geocode(agent, config)?;
    write_geocode_cache(&GeoCache {
        key,
        place: place.clone(),
    });
    Ok(place)
}

fn read_geocode_cache() -> Option<GeoCache> {
    let text = std::fs::read_to_string(GEOCODE_CACHE_FILE).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_geocode_cache(cache: &GeoCache) {
    match serde_json::to_string_pretty(cache) {
        Ok(json) => {
            if let Err(e) = std::fs::write(GEOCODE_CACHE_FILE, json) {
                eprintln!("[weather] could not write geocode cache: {e}");
            }
        }
        Err(e) => eprintln!("[weather] could not serialize geocode cache: {e}"),
    }
}

fn geocode(agent: &ureq::Agent, config: &Config) -> Result<GeoResult> {
    eprintln!("[weather] geocoding city {:?} ...", config.location.city);
    let mut response = agent
        .get(GEOCODE_URL)
        .query("name", &config.location.city)
        .query("count", "10")
        .query("language", config.language.code())
        .query("format", "json")
        .call()
        .context("geocoding request failed")?;

    let body: GeoResponse = response
        .body_mut()
        .read_json()
        .context("decoding geocoding response")?;
    eprintln!("[weather] geocoding response received");

    let results = body
        .results
        .filter(|r| !r.is_empty())
        .ok_or_else(|| anyhow!("no geocoding match for city {:?}", config.location.city))?;

    // Prefer a result matching the configured country code, else take the first.
    let chosen = match &config.location.country {
        Some(country) => results
            .iter()
            .find(|r| {
                r.country_code
                    .as_deref()
                    .is_some_and(|c| c.eq_ignore_ascii_case(country))
            })
            .or_else(|| results.first()),
        None => results.first(),
    };

    let chosen = chosen
        .cloned()
        .ok_or_else(|| anyhow!("no usable geocoding result"))?;
    eprintln!(
        "[weather] resolved to {} @ {:.4},{:.4}",
        chosen.name, chosen.latitude, chosen.longitude
    );
    Ok(chosen)
}

fn fetch_raw(
    agent: &ureq::Agent,
    config: &Config,
    latitude: f64,
    longitude: f64,
) -> Result<ForecastResponse> {
    eprintln!("[weather] fetching forecast @ {latitude:.4},{longitude:.4} ...");
    let mut response = agent
        .get(FORECAST_URL)
        .query("latitude", latitude.to_string())
        .query("longitude", longitude.to_string())
        .query("current", "temperature_2m,weather_code")
        .query(
            "daily",
            "weather_code,temperature_2m_max,temperature_2m_min,precipitation_probability_max",
        )
        .query("timezone", "auto")
        .query("forecast_days", FORECAST_DAYS.to_string())
        .query("temperature_unit", config.units.temperature.api())
        .query("wind_speed_unit", config.units.wind.api())
        .query("precipitation_unit", config.units.precipitation.api())
        .call()
        .context("forecast request failed")?;

    let parsed = response
        .body_mut()
        .read_json()
        .context("decoding forecast response")?;
    eprintln!("[weather] forecast response received");
    Ok(parsed)
}

fn build_days(daily: &DailyBlock) -> Result<Vec<Day>> {
    let len = daily.time.len();
    if len == 0 {
        return Err(anyhow!("forecast contained no daily entries"));
    }

    let mut days = Vec::with_capacity(len);
    for i in 0..len {
        let Some(date) = Date::parse(&daily.time[i]) else {
            continue;
        };
        let condition = daily
            .weather_code
            .get(i)
            .and_then(|c| *c)
            .map(|c| Condition::from_wmo(c as u8))
            .unwrap_or(Condition::Unknown);
        days.push(Day {
            date,
            temp_max: daily
                .temperature_2m_max
                .get(i)
                .and_then(|t| *t)
                .unwrap_or(0.0),
            temp_min: daily
                .temperature_2m_min
                .get(i)
                .and_then(|t| *t)
                .unwrap_or(0.0),
            precip_prob: daily
                .precipitation_probability_max
                .get(i)
                .and_then(|p| *p)
                .map(|p| p.round().clamp(0.0, 100.0) as u8),
            condition,
        });
    }

    if days.is_empty() {
        return Err(anyhow!("no parseable daily entries in forecast"));
    }
    Ok(days)
}

// --- Open-Meteo JSON wire types -------------------------------------------

#[derive(Debug, Deserialize)]
struct GeoResponse {
    results: Option<Vec<GeoResult>>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct GeoResult {
    name: String,
    latitude: f64,
    longitude: f64,
    country: Option<String>,
    country_code: Option<String>,
}

/// Cache key: re-geocode only if the city, country filter, or language changes.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
struct GeoKey {
    city: String,
    country: Option<String>,
    language: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct GeoCache {
    key: GeoKey,
    place: GeoResult,
}

#[derive(Debug, Deserialize)]
struct ForecastResponse {
    #[serde(default)]
    utc_offset_seconds: i64,
    current: CurrentBlock,
    daily: DailyBlock,
}

#[derive(Debug, Deserialize)]
struct CurrentBlock {
    temperature_2m: f64,
    weather_code: i64,
}

#[derive(Debug, Deserialize)]
struct DailyBlock {
    time: Vec<String>,
    weather_code: Vec<Option<i64>>,
    temperature_2m_max: Vec<Option<f64>>,
    temperature_2m_min: Vec<Option<f64>>,
    #[serde(default)]
    precipitation_probability_max: Vec<Option<f64>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = r#"{
        "current": {"time": "2026-06-17T12:00", "temperature_2m": 21.4, "weather_code": 2},
        "daily": {
            "time": ["2026-06-17", "2026-06-18", "2026-06-19"],
            "weather_code": [2, 61, 95],
            "temperature_2m_max": [25.1, 22.0, 19.5],
            "temperature_2m_min": [14.2, 13.0, 12.1],
            "precipitation_probability_max": [10, 80, null]
        }
    }"#;

    #[test]
    fn parses_forecast_fixture() {
        let raw: ForecastResponse = serde_json::from_str(FIXTURE).unwrap();
        assert_eq!(raw.current.weather_code, 2);

        let days = build_days(&raw.daily).unwrap();
        assert_eq!(days.len(), 3);
        assert_eq!(days[0].condition, Condition::PartlyCloudy);
        assert_eq!(days[1].condition, Condition::Rain);
        assert_eq!(days[1].precip_prob, Some(80));
        assert_eq!(days[2].precip_prob, None);
        assert_eq!(
            days[0].date.weekday_short(crate::config::Language::En),
            "Wed"
        );
    }
}
