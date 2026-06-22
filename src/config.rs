//! User configuration, loaded from a TOML file.
//!
//! Units map directly onto Open-Meteo API query values, so the same enum drives
//! both the network request and the on-screen unit symbols.

use std::path::Path;

use anyhow::{Context, Result};
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct Config {
    pub location: Location,
    /// Weather data source.
    #[serde(default)]
    pub provider: WeatherProvider,
    /// UI language for labels, weekdays and months.
    #[serde(default)]
    pub language: Language,
    #[serde(default)]
    pub units: Units,
    #[serde(default)]
    pub display: Display,
    #[serde(default)]
    pub refresh: Refresh,
}

/// Weather data source. Open-Meteo needs no key; met.no (Norwegian
/// Meteorological Institute, the data behind Yr.no) also needs no key but
/// sends a descriptive User-Agent and derives the local timezone from the
/// geocoded coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum WeatherProvider {
    #[default]
    OpenMeteo,
    MetNo,
}

/// Display language. Croatian uses ASCII counterparts for diacritics
/// (c/z/s/d for č/ž/š/đ), which the bitmap fonts render cleanly.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    #[default]
    En,
    Hr,
}

impl Language {
    /// ISO code for API query parameters (Open-Meteo geocoding `language`).
    pub fn code(self) -> &'static str {
        match self {
            Language::En => "en",
            Language::Hr => "hr",
        }
    }

    /// Prefix for the current-conditions line, e.g. "Now:".
    pub fn now_prefix(self) -> &'static str {
        match self {
            Language::En => "Now:",
            Language::Hr => "Sada:",
        }
    }

    /// Prefix for the refresh-time line, e.g. "Refreshed".
    pub fn refreshed_prefix(self) -> &'static str {
        match self {
            Language::En => "Refreshed",
            Language::Hr => "Osvjezeno",
        }
    }
}

impl Config {
    pub fn load(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        let text = std::fs::read_to_string(path)
            .with_context(|| format!("reading config file {}", path.display()))?;
        let config: Config =
            toml::from_str(&text).with_context(|| format!("parsing config {}", path.display()))?;
        Ok(config)
    }
}

#[derive(Debug, Clone, Deserialize)]
pub struct Location {
    /// City name used for geocoding, e.g. "Zagreb".
    pub city: String,
    /// Optional ISO country code (e.g. "HR") to disambiguate the geocoding hit.
    #[serde(default)]
    pub country: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Units {
    pub temperature: TemperatureUnit,
    pub wind: WindUnit,
    pub precipitation: PrecipitationUnit,
}

impl Default for Units {
    fn default() -> Self {
        Self {
            temperature: TemperatureUnit::Celsius,
            wind: WindUnit::Kmh,
            precipitation: PrecipitationUnit::Mm,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TemperatureUnit {
    Celsius,
    Fahrenheit,
}

impl TemperatureUnit {
    /// Value for the Open-Meteo `temperature_unit` query parameter.
    pub fn api(self) -> &'static str {
        match self {
            TemperatureUnit::Celsius => "celsius",
            TemperatureUnit::Fahrenheit => "fahrenheit",
        }
    }

    pub fn symbol(self) -> &'static str {
        match self {
            TemperatureUnit::Celsius => "°C",
            TemperatureUnit::Fahrenheit => "°F",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WindUnit {
    Kmh,
    Ms,
    Mph,
    Kn,
}

impl WindUnit {
    /// Value for the Open-Meteo `wind_speed_unit` query parameter.
    pub fn api(self) -> &'static str {
        match self {
            WindUnit::Kmh => "kmh",
            WindUnit::Ms => "ms",
            WindUnit::Mph => "mph",
            WindUnit::Kn => "kn",
        }
    }

    pub fn symbol(self) -> &'static str {
        match self {
            WindUnit::Kmh => "km/h",
            WindUnit::Ms => "m/s",
            WindUnit::Mph => "mph",
            WindUnit::Kn => "kn",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PrecipitationUnit {
    Mm,
    Inch,
}

impl PrecipitationUnit {
    /// Value for the Open-Meteo `precipitation_unit` query parameter.
    pub fn api(self) -> &'static str {
        match self {
            PrecipitationUnit::Mm => "mm",
            PrecipitationUnit::Inch => "inch",
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct Display {
    /// epd-waveshare module identifier, e.g. "epd7in5_v2".
    pub model: String,
    /// Rotation in degrees (0, 90, 180, 270).
    pub rotation: u16,
    /// Swap black/white if the panel renders colors inverted (background black).
    pub invert: bool,
    pub pins: Pins,
}

impl Default for Display {
    fn default() -> Self {
        Self {
            model: "epd7in5_v2".to_string(),
            rotation: 0,
            invert: false,
            pins: Pins::default(),
        }
    }
}

/// BCM GPIO pin numbers for the standard Waveshare e-Paper HAT wiring.
/// Chip-select uses SPI0 CE0 and is handled by the SPI peripheral.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(default)]
pub struct Pins {
    pub reset: u8,
    pub dc: u8,
    pub busy: u8,
    /// Power-enable pin. Newer HATs gate panel power on this line; it must be
    /// driven high before the controller will respond. Set to 255 to disable.
    pub power: u8,
}

impl Default for Pins {
    fn default() -> Self {
        Self {
            reset: 17,
            dc: 25,
            busy: 24,
            power: 18,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(default)]
pub struct Refresh {
    pub interval_minutes: u32,
}

impl Default for Refresh {
    fn default() -> Self {
        Self {
            interval_minutes: 60,
        }
    }
}
