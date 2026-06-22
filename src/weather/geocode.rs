//! City → coordinates resolution via Open-Meteo's geocoder, shared by every
//! weather provider and cached on disk so geocoding runs once per location.
//! No API key required.

use anyhow::{Context, Result, anyhow};
use serde::{Deserialize, Serialize};

use crate::config::{Config, Language};

const GEOCODE_URL: &str = "https://geocoding-api.open-meteo.com/v1/search";
/// On-disk cache of the resolved coordinates (in the working directory).
const GEOCODE_CACHE_FILE: &str = "geocode_cache.json";

/// A resolved location: coordinates plus the metadata providers need.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Place {
    pub name: String,
    pub latitude: f64,
    pub longitude: f64,
    pub country: Option<String>,
    pub country_code: Option<String>,
    /// IANA timezone name (e.g. "Europe/Zagreb"). Providers that return UTC
    /// timestamps use it to derive local time. Defaulted for cache entries
    /// written before this field existed.
    #[serde(default)]
    pub timezone: Option<String>,
}

/// Resolve coordinates for the configured city, reusing the on-disk cache when
/// it matches the current city/country/language; otherwise geocode and cache.
pub fn resolve_place(agent: &ureq::Agent, config: &Config) -> Result<Place> {
    let key = GeoKey {
        city: config.location.city.clone(),
        country: config.location.country.clone(),
        language: config.language.code().to_string(),
    };

    if let Some(cached) = read_cache() {
        // Re-geocode if the cache predates the timezone field, so met.no gets a
        // real local zone instead of silently falling back to UTC.
        if cached.key == key && cached.place.timezone.is_some() {
            eprintln!("[weather] using cached coordinates for {:?}", key.city);
            return Ok(cached.place);
        }
    }

    let place = geocode(agent, config)?;
    write_cache(&GeoCache {
        key,
        place: place.clone(),
    });
    Ok(place)
}

/// Preferred display name for the place, e.g. "Zagreb, Croatia".
pub fn display_name(place: &Place, lang: Language) -> String {
    match localized_country(
        place.country_code.as_deref(),
        place.country.as_deref(),
        lang,
    ) {
        Some(country) => format!("{}, {}", place.name, country),
        None => place.name.clone(),
    }
}

/// Preferred display name for a country. The geocoder's localized name can be
/// formal (e.g. "Republika Hrvatska"); override the cases we care about.
fn localized_country(code: Option<&str>, api_name: Option<&str>, lang: Language) -> Option<String> {
    if lang == Language::Hr && code.is_some_and(|c| c.eq_ignore_ascii_case("HR")) {
        return Some("Hrvatska".to_string());
    }
    api_name.map(str::to_string)
}

fn read_cache() -> Option<GeoCache> {
    let text = std::fs::read_to_string(GEOCODE_CACHE_FILE).ok()?;
    serde_json::from_str(&text).ok()
}

fn write_cache(cache: &GeoCache) {
    match serde_json::to_string_pretty(cache) {
        Ok(json) => {
            if let Err(e) = std::fs::write(GEOCODE_CACHE_FILE, json) {
                eprintln!("[weather] could not write geocode cache: {e}");
            }
        }
        Err(e) => eprintln!("[weather] could not serialize geocode cache: {e}"),
    }
}

fn geocode(agent: &ureq::Agent, config: &Config) -> Result<Place> {
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

// --- Open-Meteo geocoding JSON wire types ---------------------------------

#[derive(Debug, Deserialize)]
struct GeoResponse {
    results: Option<Vec<Place>>,
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
    place: Place,
}
