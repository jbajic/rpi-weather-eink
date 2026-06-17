//! Weather data: fetching from Open-Meteo and the display-facing domain model.

mod model;
mod open_meteo;

pub use model::{Condition, Current, Date, Day, Forecast};
pub use open_meteo::fetch_forecast;
