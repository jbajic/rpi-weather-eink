//! A simple 1-bit in-memory framebuffer used for the host (PNG) render path.
//!
//! Implements [`DrawTarget`] over [`BinaryColor`] exactly like the e-paper
//! display buffer, so [`crate::render::draw`] is identical on host and device.

use std::path::Path;

use anyhow::{Context, Result};
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;

pub struct Canvas {
    width: u32,
    height: u32,
    /// One entry per pixel; `true` means black (foreground / "on").
    pixels: Vec<bool>,
}

impl Canvas {
    /// Create a white canvas of the given size.
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            pixels: vec![false; (width * height) as usize],
        }
    }

    pub fn width(&self) -> u32 {
        self.width
    }

    pub fn height(&self) -> u32 {
        self.height
    }

    /// Whether the pixel is foreground (black). Out-of-bounds reads as white.
    pub fn is_on(&self, x: u32, y: u32) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }
        self.pixels[(y * self.width + x) as usize]
    }

    /// Write the framebuffer to a 1-bit-style grayscale PNG (black on white).
    pub fn save_png(&self, path: impl AsRef<Path>) -> Result<()> {
        let mut img = image::GrayImage::new(self.width, self.height);
        for (i, &on) in self.pixels.iter().enumerate() {
            let x = (i as u32) % self.width;
            let y = (i as u32) / self.width;
            let value = if on { 0 } else { 255 };
            img.put_pixel(x, y, image::Luma([value]));
        }
        let path = path.as_ref();
        img.save(path)
            .with_context(|| format!("writing PNG {}", path.display()))?;
        Ok(())
    }
}

impl OriginDimensions for Canvas {
    fn size(&self) -> Size {
        Size::new(self.width, self.height)
    }
}

impl DrawTarget for Canvas {
    type Color = BinaryColor;
    type Error = core::convert::Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        for Pixel(coord, color) in pixels {
            let (Ok(x), Ok(y)) = (usize::try_from(coord.x), usize::try_from(coord.y)) else {
                continue;
            };
            if x < self.width as usize && y < self.height as usize {
                self.pixels[y * self.width as usize + x] = color.is_on();
            }
        }
        Ok(())
    }
}
