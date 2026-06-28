//! Long-running daemon: initialise the panel once, then refresh on an interval.
//!
//! Keeping a single [`Panel`] alive (rather than re-initialising per render)
//! means the controller is set up once and never deep-slept/re-woken between
//! refreshes. The refresh cadence comes from `[refresh] interval_minutes`.
//!
//! Only built with `--features device` (see `required-features` in Cargo.toml).

use std::process::ExitCode;
use std::sync::Arc;
use std::time::Duration;

use anyhow::{Context, Result};
use eink::canvas::Canvas;
use eink::health::Health;
use eink::output::epaper::{self, Panel};
use eink::{Config, render, weather};

fn parse_config_path() -> String {
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        if matches!(arg.as_str(), "--config" | "-c") {
            if let Some(v) = it.next() {
                return v;
            }
        }
    }
    "config.toml".to_string()
}

/// Fetch the forecast and render it into a fresh canvas.
fn render_canvas(config: &Config) -> Result<Canvas> {
    let forecast = weather::fetch_forecast(config).context("fetching forecast")?;
    let (width, height) = epaper::canvas_size(config);
    eprintln!("[eink] rendering {width}x{height} framebuffer ...");
    let mut canvas = Canvas::new(width, height);
    render::draw(&mut canvas, &forecast, config)?;
    eprintln!("[eink] framebuffer rendered: {}", forecast.location_name);
    Ok(canvas)
}

fn run() -> Result<()> {
    let config_path = parse_config_path();
    eprintln!("[eink] loading config {config_path} ...");
    let config = Config::load(&config_path)?;

    let minutes = config.refresh.interval_minutes.max(1);
    let interval = Duration::from_secs(u64::from(minutes) * 60);
    eprintln!("[eink] refresh interval: {minutes} min");

    let health = Arc::new(Health::default());
    if config.health.enabled {
        // Stale after 2.5 refresh intervals: tolerates one missed tick.
        let stale = (u64::from(minutes) * 60 * 5) / 2;
        match eink::health::serve(&config.health.listen, health.clone(), stale) {
            Ok(()) => eprintln!("[eink] health endpoint on {}", config.health.listen),
            Err(e) => eprintln!("[eink] health endpoint failed to bind: {e:#}"),
        }
    }

    eprintln!("[eink] initialising panel (one time) ...");
    let mut panel = Panel::open(&config).context("opening panel")?;
    eprintln!("[eink] panel ready; entering refresh loop");

    loop {
        match render_canvas(&config) {
            Ok(canvas) => match panel.push(&canvas) {
                Ok(()) => health.mark_success(),
                Err(e) => eprintln!("[eink] panel push failed: {e:#}"),
            },
            Err(e) => eprintln!("[eink] refresh failed (will retry next tick): {e:#}"),
        }
        eprintln!("[eink] sleeping {minutes} min until next refresh");
        std::thread::sleep(interval);
    }
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}
