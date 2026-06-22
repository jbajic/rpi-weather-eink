//! Weather data: fetching from a configurable provider and the display-facing
//! domain model.

mod geocode;
mod met_no;
mod model;
mod open_meteo;

pub use model::{Condition, Current, Date, Day, Forecast};

use std::time::Duration;

use anyhow::Result;

use crate::config::{Config, WeatherProvider};

/// Cap each HTTP request so a flaky network can't hang the daemon; on timeout
/// the call errors and the daemon simply retries on the next refresh tick.
const HTTP_TIMEOUT: Duration = Duration::from_secs(20);

/// Resolve the configured city and fetch its weekly forecast from the
/// configured provider.
pub fn fetch_forecast(config: &Config) -> Result<Forecast> {
    match config.provider {
        WeatherProvider::OpenMeteo => open_meteo::fetch_forecast(config),
        WeatherProvider::MetNo => met_no::fetch_forecast(config),
    }
}

pub(crate) fn http_agent() -> ureq::Agent {
    ureq::Agent::new_with_config(
        ureq::Agent::config_builder()
            .timeout_global(Some(HTTP_TIMEOUT))
            .build(),
    )
}
