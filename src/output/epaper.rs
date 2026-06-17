//! Drive the Waveshare 7.5" V2 e-paper panel from a Raspberry Pi over SPI.
//!
//! The render pipeline produces a [`Canvas`] (1-bit, `BinaryColor`); this module
//! blits it into the panel's native buffer and pushes a single full refresh,
//! then sends the controller to deep sleep to save power between runs.

use anyhow::{Context, Result, anyhow};
use embedded_graphics::Pixel;
use embedded_graphics::prelude::*;
use epd_waveshare::color::Color;
use epd_waveshare::epd7in5_v2::{self, Display7in5, Epd7in5};
use epd_waveshare::graphics::DisplayRotation;
use epd_waveshare::prelude::WaveshareDisplay;
use rppal::gpio::Gpio;
use rppal::hal::Delay;
use rppal::spi::{Bus, Mode, SimpleHalSpiDevice, SlaveSelect, Spi};

use crate::canvas::Canvas;
use crate::config::Config;

const SPI_CLOCK_HZ: u32 = 4_000_000;

/// Logical canvas dimensions for the configured rotation. The render layout
/// uses these so the panel's own rotation mapping lines up 1:1.
pub fn canvas_size(config: &Config) -> (u32, u32) {
    match config.display.rotation {
        90 | 270 => (epd7in5_v2::HEIGHT, epd7in5_v2::WIDTH),
        _ => (epd7in5_v2::WIDTH, epd7in5_v2::HEIGHT),
    }
}

/// Push an already-rendered canvas to the panel, then put it to deep sleep.
pub fn show(config: &Config, canvas: &Canvas) -> Result<()> {
    let bus = Spi::new(Bus::Spi0, SlaveSelect::Ss0, SPI_CLOCK_HZ, Mode::Mode0)
        .context("opening SPI0 (is SPI enabled via raspi-config?)")?;
    let mut spi = SimpleHalSpiDevice::new(bus);
    let mut delay = Delay::new();

    let gpio = Gpio::new().context("opening GPIO")?;
    let dc = gpio.get(config.display.pins.dc).context("DC pin")?.into_output();
    let rst = gpio
        .get(config.display.pins.reset)
        .context("RESET pin")?
        .into_output();
    let busy = gpio
        .get(config.display.pins.busy)
        .context("BUSY pin")?
        .into_input();

    let mut epd = Epd7in5::new(&mut spi, busy, dc, rst, &mut delay, None)
        .map_err(|e| anyhow!("initialising e-paper: {e:?}"))?;

    let mut display = Display7in5::default();
    display.set_rotation(rotation(config.display.rotation));
    blit(canvas, &mut display);

    epd.update_and_display_frame(&mut spi, display.buffer(), &mut delay)
        .map_err(|e| anyhow!("sending frame to panel: {e:?}"))?;
    epd.sleep(&mut spi, &mut delay)
        .map_err(|e| anyhow!("putting panel to sleep: {e:?}"))?;
    Ok(())
}

fn rotation(degrees: u16) -> DisplayRotation {
    match degrees {
        90 => DisplayRotation::Rotate90,
        180 => DisplayRotation::Rotate180,
        270 => DisplayRotation::Rotate270,
        _ => DisplayRotation::Rotate0,
    }
}

fn blit(canvas: &Canvas, display: &mut Display7in5) {
    for y in 0..canvas.height() {
        for x in 0..canvas.width() {
            let color = if canvas.is_on(x, y) {
                Color::Black
            } else {
                Color::White
            };
            // Drawing into the in-memory buffer is infallible.
            let _ = Pixel(Point::new(x as i32, y as i32), color).draw(display);
        }
    }
}
