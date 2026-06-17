//! Weather icons drawn as 1-bit vector primitives.
//!
//! Drawing them from primitives (rather than embedding bitmaps) keeps the
//! binary self-contained and scales cleanly to any card size. They render as
//! solid black silhouettes, which read well on a black/white panel.

use anyhow::{Result, anyhow};
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Circle, Line, PrimitiveStyle, Rectangle, Triangle};

use crate::weather::Condition;

const BLACK: BinaryColor = BinaryColor::On;

/// Draw `condition` centered on `center`, sized to fit a `size`×`size` box.
pub fn draw_icon<D>(target: &mut D, condition: Condition, center: Point, size: u32) -> Result<()>
where
    D: DrawTarget<Color = BinaryColor>,
    D::Error: core::fmt::Debug,
{
    let s = size as f32;
    match condition {
        Condition::Clear | Condition::MainlyClear => sun(target, center, s * 0.32)?,
        Condition::PartlyCloudy => {
            sun(target, center + Point::new(-(s * 0.18) as i32, -(s * 0.18) as i32), s * 0.20)?;
            cloud(target, center + Point::new((s * 0.10) as i32, (s * 0.10) as i32), s * 0.80)?;
        }
        Condition::Overcast | Condition::Fog => cloud(target, center, s * 0.92)?,
        Condition::Drizzle => {
            cloud(target, center + Point::new(0, -(s * 0.12) as i32), s * 0.82)?;
            drops(target, center, s, 3, s * 0.10)?;
        }
        Condition::Rain | Condition::Showers => {
            cloud(target, center + Point::new(0, -(s * 0.12) as i32), s * 0.82)?;
            rain(target, center, s, 4)?;
        }
        Condition::FreezingRain => {
            cloud(target, center + Point::new(0, -(s * 0.12) as i32), s * 0.82)?;
            rain(target, center, s, 3)?;
            drops(target, center + Point::new((s * 0.22) as i32, 0), s, 1, s * 0.07)?;
        }
        Condition::Snow | Condition::SnowShowers => {
            cloud(target, center + Point::new(0, -(s * 0.12) as i32), s * 0.82)?;
            snow(target, center, s)?;
        }
        Condition::Thunderstorm => {
            cloud(target, center + Point::new(0, -(s * 0.12) as i32), s * 0.82)?;
            bolt(target, center, s)?;
        }
        Condition::Unknown => cloud(target, center, s * 0.80)?,
    }
    Ok(())
}

fn err<E: core::fmt::Debug>(e: E) -> anyhow::Error {
    anyhow!("icon draw error: {e:?}")
}

fn filled_circle<D>(target: &mut D, center: Point, radius: f32) -> Result<()>
where
    D: DrawTarget<Color = BinaryColor>,
    D::Error: core::fmt::Debug,
{
    let r = radius.max(1.0);
    let d = (r * 2.0) as u32;
    let top_left = Point::new(center.x - r as i32, center.y - r as i32);
    Circle::new(top_left, d)
        .into_styled(PrimitiveStyle::with_fill(BLACK))
        .draw(target)
        .map_err(err)?;
    Ok(())
}

fn line<D>(target: &mut D, a: Point, b: Point, width: u32) -> Result<()>
where
    D: DrawTarget<Color = BinaryColor>,
    D::Error: core::fmt::Debug,
{
    Line::new(a, b)
        .into_styled(PrimitiveStyle::with_stroke(BLACK, width.max(1)))
        .draw(target)
        .map_err(err)?;
    Ok(())
}

fn sun<D>(target: &mut D, center: Point, radius: f32) -> Result<()>
where
    D: DrawTarget<Color = BinaryColor>,
    D::Error: core::fmt::Debug,
{
    filled_circle(target, center, radius)?;
    let ray_in = radius * 1.35;
    let ray_out = radius * 1.95;
    let width = (radius * 0.22).max(2.0) as u32;
    for k in 0..8 {
        let angle = k as f32 * std::f32::consts::FRAC_PI_4;
        let (sin, cos) = angle.sin_cos();
        let a = Point::new(
            center.x + (cos * ray_in) as i32,
            center.y + (sin * ray_in) as i32,
        );
        let b = Point::new(
            center.x + (cos * ray_out) as i32,
            center.y + (sin * ray_out) as i32,
        );
        line(target, a, b, width)?;
    }
    Ok(())
}

/// A solid cloud silhouette roughly `width` px wide, sitting on `center`.
fn cloud<D>(target: &mut D, center: Point, width: f32) -> Result<()>
where
    D: DrawTarget<Color = BinaryColor>,
    D::Error: core::fmt::Debug,
{
    let cx = center.x as f32;
    let cy = center.y as f32;
    filled_circle(target, Point::new((cx - width * 0.24) as i32, cy as i32), width * 0.20)?;
    filled_circle(
        target,
        Point::new(cx as i32, (cy - width * 0.12) as i32),
        width * 0.27,
    )?;
    filled_circle(target, Point::new((cx + width * 0.26) as i32, cy as i32), width * 0.22)?;
    let base = Rectangle::new(
        Point::new((cx - width * 0.44) as i32, cy as i32),
        Size::new((width * 0.88) as u32, (width * 0.22) as u32),
    );
    base.into_styled(PrimitiveStyle::with_fill(BLACK))
        .draw(target)
        .map_err(err)?;
    Ok(())
}

/// Short diagonal rain streaks below the cloud.
fn rain<D>(target: &mut D, center: Point, s: f32, count: u32)
    -> Result<()>
where
    D: DrawTarget<Color = BinaryColor>,
    D::Error: core::fmt::Debug,
{
    let top = center.y as f32 + s * 0.14;
    let len = s * 0.22;
    let width = (s * 0.05).max(2.0) as u32;
    let spread = s * 0.5;
    for k in 0..count {
        let frac = if count > 1 { k as f32 / (count - 1) as f32 } else { 0.5 };
        let x = center.x as f32 - spread / 2.0 + spread * frac;
        let a = Point::new(x as i32, top as i32);
        let b = Point::new((x - len * 0.4) as i32, (top + len) as i32);
        line(target, a, b, width)?;
    }
    Ok(())
}

/// Small round droplets (drizzle / freezing rain).
fn drops<D>(target: &mut D, center: Point, s: f32, count: u32, radius: f32) -> Result<()>
where
    D: DrawTarget<Color = BinaryColor>,
    D::Error: core::fmt::Debug,
{
    let y = center.y as f32 + s * 0.22;
    let spread = s * 0.4;
    for k in 0..count {
        let frac = if count > 1 { k as f32 / (count - 1) as f32 } else { 0.5 };
        let x = center.x as f32 - spread / 2.0 + spread * frac;
        filled_circle(target, Point::new(x as i32, y as i32), radius)?;
    }
    Ok(())
}

/// Snowflakes drawn as small crosses below the cloud.
fn snow<D>(target: &mut D, center: Point, s: f32) -> Result<()>
where
    D: DrawTarget<Color = BinaryColor>,
    D::Error: core::fmt::Debug,
{
    let y = center.y as f32 + s * 0.22;
    let arm = s * 0.07;
    let width = (s * 0.04).max(1.0) as u32;
    for k in 0..3 {
        let x = center.x as f32 - s * 0.2 + s * 0.2 * k as f32;
        let c = Point::new(x as i32, y as i32);
        line(
            target,
            Point::new(c.x - arm as i32, c.y),
            Point::new(c.x + arm as i32, c.y),
            width,
        )?;
        line(
            target,
            Point::new(c.x, c.y - arm as i32),
            Point::new(c.x, c.y + arm as i32),
            width,
        )?;
    }
    Ok(())
}

/// A lightning bolt below the cloud.
fn bolt<D>(target: &mut D, center: Point, s: f32) -> Result<()>
where
    D: DrawTarget<Color = BinaryColor>,
    D::Error: core::fmt::Debug,
{
    let cx = center.x as f32;
    let top = center.y as f32 + s * 0.12;
    let p1 = Point::new((cx + s * 0.06) as i32, top as i32);
    let p2 = Point::new((cx - s * 0.14) as i32, (top + s * 0.22) as i32);
    let p3 = Point::new((cx + s * 0.02) as i32, (top + s * 0.22) as i32);
    let p4 = Point::new((cx - s * 0.10) as i32, (top + s * 0.44) as i32);
    Triangle::new(p1, p2, p3)
        .into_styled(PrimitiveStyle::with_fill(BLACK))
        .draw(target)
        .map_err(err)?;
    line(target, p3, p4, (s * 0.06).max(2.0) as u32)?;
    Ok(())
}
