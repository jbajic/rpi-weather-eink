//! Drive the Waveshare 7.5" V2 e-paper panel from a Raspberry Pi over SPI.
//!
//! [`Panel`] opens SPI/GPIO and initialises the controller **once**, then
//! [`Panel::push`] sends a full-refresh frame. A long-running daemon keeps a
//! single `Panel` alive and pushes on each tick — so the controller is only
//! initialised once and never deep-slept/re-woken between refreshes, which is
//! the cycle that can hang on BUSY.
//!
//! [`show`] is the one-shot convenience: open, push once, drop.

use anyhow::{Context, Result, anyhow};
use embedded_graphics::Pixel;
use embedded_graphics::prelude::*;
use epd_waveshare::color::Color;
use epd_waveshare::epd7in5_v2::{self, Display7in5, Epd7in5};
use epd_waveshare::graphics::DisplayRotation;
use epd_waveshare::prelude::WaveshareDisplay;
use rppal::gpio::{Gpio, InputPin, OutputPin};
use rppal::hal::Delay;
use rppal::spi::{Bus, Mode, SimpleHalSpiDevice, SlaveSelect, Spi};

use crate::canvas::Canvas;
use crate::config::Config;

const SPI_CLOCK_HZ: u32 = 4_000_000;

type PanelEpd = Epd7in5<SimpleHalSpiDevice, InputPin, OutputPin, OutputPin, Delay>;

/// Logical canvas dimensions for the configured rotation. The render layout
/// uses these so the panel's own rotation mapping lines up 1:1.
pub fn canvas_size(config: &Config) -> (u32, u32) {
    match config.display.rotation {
        90 | 270 => (epd7in5_v2::HEIGHT, epd7in5_v2::WIDTH),
        _ => (epd7in5_v2::WIDTH, epd7in5_v2::HEIGHT),
    }
}

/// An initialised panel, ready to receive frames.
pub struct Panel {
    spi: SimpleHalSpiDevice,
    delay: Delay,
    epd: PanelEpd,
    rotation: DisplayRotation,
    invert: bool,
}

impl Panel {
    /// Open SPI/GPIO and initialise the controller (resets + powers on).
    pub fn open(config: &Config) -> Result<Self> {
        log("opening SPI0 ...");
        let bus = Spi::new(Bus::Spi0, SlaveSelect::Ss0, SPI_CLOCK_HZ, Mode::Mode0)
            .context("opening SPI0 (is SPI enabled via raspi-config?)")?;
        let mut spi = SimpleHalSpiDevice::new(bus);
        let mut delay = Delay::new();
        log("SPI0 open");

        log("opening GPIO pins ...");
        let gpio = Gpio::new().context("opening GPIO")?;
        let dc = gpio
            .get(config.display.pins.dc)
            .context("DC pin")?
            .into_output();
        let rst = gpio
            .get(config.display.pins.reset)
            .context("RESET pin")?
            .into_output();
        let busy = gpio
            .get(config.display.pins.busy)
            .context("BUSY pin")?
            .into_input();
        log("GPIO pins open");

        log("initialising controller (reset + power-on, waits on BUSY) ...");
        let epd = Epd7in5::new(&mut spi, busy, dc, rst, &mut delay, None)
            .map_err(|e| anyhow!("initialising e-paper: {e:?}"))?;
        log("controller ready");

        Ok(Self {
            spi,
            delay,
            epd,
            rotation: rotation(config.display.rotation),
            invert: config.display.invert,
        })
    }

    /// Render `canvas` into the panel buffer and do a full refresh.
    pub fn push(&mut self, canvas: &Canvas) -> Result<()> {
        let mut display = Display7in5::default();
        display.set_rotation(self.rotation);
        log("blitting framebuffer to panel buffer ...");
        blit(canvas, &mut display, self.invert);
        log("blit complete");

        log("pushing frame + refreshing (waits on BUSY) ...");
        self.epd
            .update_and_display_frame(&mut self.spi, display.buffer(), &mut self.delay)
            .map_err(|e| anyhow!("sending frame to panel: {e:?}"))?;
        log("frame displayed");
        Ok(())
    }
}

/// One-shot: open the panel, push a single frame, and drop it.
pub fn show(config: &Config, canvas: &Canvas) -> Result<()> {
    let mut panel = Panel::open(config)?;
    panel.push(canvas)?;
    Ok(())
}

/// Progress log to stderr (captured by systemd's journal).
fn log(msg: &str) {
    eprintln!("[panel] {msg}");
}

fn rotation(degrees: u16) -> DisplayRotation {
    match degrees {
        90 => DisplayRotation::Rotate90,
        180 => DisplayRotation::Rotate180,
        270 => DisplayRotation::Rotate270,
        _ => DisplayRotation::Rotate0,
    }
}

fn blit(canvas: &Canvas, display: &mut Display7in5, invert: bool) {
    for y in 0..canvas.height() {
        for x in 0..canvas.width() {
            // Foreground pixels are black; `invert` flips the polarity for panels
            // that render colors reversed.
            let as_black = canvas.is_on(x, y) != invert;
            let color = if as_black { Color::Black } else { Color::White };
            // Drawing into the in-memory buffer is infallible.
            let _ = Pixel(Point::new(x as i32, y as i32), color).draw(display);
        }
    }
}
