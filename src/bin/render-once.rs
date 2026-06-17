//! One-shot renderer: load config, fetch the forecast, render it once.
//!
//! On the default (host) build it writes a PNG preview. On a `device` build it
//! drives the e-paper panel, and still writes a PNG if `--output` is given.

use std::process::ExitCode;

use anyhow::{Context, Result};
use eink::canvas::Canvas;
use eink::{Config, render, weather};

#[cfg(not(feature = "device"))]
const WIDTH: u32 = 800;
#[cfg(not(feature = "device"))]
const HEIGHT: u32 = 480;

struct Args {
    config: String,
    output: Option<String>,
}

fn parse_args() -> Args {
    let mut config = "config.toml".to_string();
    let mut output = None;
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--config" | "-c" => {
                if let Some(v) = it.next() {
                    config = v;
                }
            }
            "--output" | "-o" => output = it.next(),
            other => eprintln!("warning: ignoring unknown argument {other:?}"),
        }
    }
    Args { config, output }
}

fn run() -> Result<()> {
    let args = parse_args();

    eprintln!("[eink] loading config {} ...", args.config);
    let config = Config::load(&args.config)?;
    eprintln!("[eink] config loaded");

    let forecast = weather::fetch_forecast(&config).context("fetching forecast")?;

    #[cfg(feature = "device")]
    let (width, height) = eink::output::epaper::canvas_size(&config);
    #[cfg(not(feature = "device"))]
    let (width, height) = (WIDTH, HEIGHT);

    eprintln!("[eink] rendering {width}x{height} framebuffer ...");
    let mut canvas = Canvas::new(width, height);
    render::draw(&mut canvas, &forecast, &config)?;
    eprintln!("[eink] framebuffer rendered");

    #[cfg(feature = "device")]
    {
        eprintln!("[eink] sending to e-paper panel");
        eink::output::epaper::show(&config, &canvas).context("driving e-paper panel")?;
        println!(
            "rendered {} ({} days) -> e-paper panel",
            forecast.location_name,
            forecast.days.len()
        );
    }

    if let Some(path) = output_path(&args) {
        canvas.save_png(&path).context("saving PNG preview")?;
        println!(
            "rendered {} ({} days) -> {}",
            forecast.location_name,
            forecast.days.len(),
            path
        );
    }
    Ok(())
}

/// PNG destination: explicit `--output` always wins; on host builds we default
/// to `forecast.png` so a bare run still produces something to look at.
fn output_path(args: &Args) -> Option<String> {
    if args.output.is_some() {
        return args.output.clone();
    }
    default_output()
}

#[cfg(not(feature = "device"))]
fn default_output() -> Option<String> {
    Some("forecast.png".to_string())
}

#[cfg(feature = "device")]
fn default_output() -> Option<String> {
    None
}

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("error: {err:#}");
            ExitCode::FAILURE
        }
    }
}
