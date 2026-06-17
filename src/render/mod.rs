//! Layout and drawing of the weather screen onto any 1-bit draw target.
//!
//! [`draw`] is generic over the draw target, so the identical layout renders to
//! the host PNG [`Canvas`](crate::canvas::Canvas) and to the e-paper buffer.

mod icons;

use anyhow::{Result, anyhow};
use embedded_graphics::pixelcolor::BinaryColor;
use embedded_graphics::prelude::*;
use embedded_graphics::primitives::{Line, PrimitiveStyle};
use u8g2_fonts::types::{FontColor, HorizontalAlignment, VerticalPosition};
use u8g2_fonts::{FontRenderer, fonts};

use crate::config::{Config, Language};
use crate::weather::{Day, Forecast};

const BLACK: BinaryColor = BinaryColor::On;
const HEADER_HEIGHT: i32 = 118;

/// Render the full forecast onto `target`. The target's own size drives the
/// layout, so rotation handled by the display buffer is transparent here.
pub fn draw<D>(target: &mut D, forecast: &Forecast, config: &Config) -> Result<()>
where
    D: DrawTarget<Color = BinaryColor> + OriginDimensions,
    D::Error: core::fmt::Debug,
{
    // Clear to white.
    target
        .clear(BinaryColor::Off)
        .map_err(|e| anyhow!("clear failed: {e:?}"))?;

    let size = target.size();
    let width = size.width as i32;
    let lang = config.language;

    draw_header(target, forecast, width, lang)?;
    hline(target, 0, width, HEADER_HEIGHT)?;
    draw_day_cards(target, &forecast.days, size, lang)?;

    Ok(())
}

fn draw_header<D>(target: &mut D, forecast: &Forecast, width: i32, lang: Language) -> Result<()>
where
    D: DrawTarget<Color = BinaryColor>,
    D::Error: core::fmt::Debug,
{
    let title = FontRenderer::new::<fonts::u8g2_font_helvB18_tf>();
    let body = FontRenderer::new::<fonts::u8g2_font_helvR14_tf>();
    let big = FontRenderer::new::<fonts::u8g2_font_logisoso32_tf>();
    let small = FontRenderer::new::<fonts::u8g2_font_helvR10_tf>();

    // Left column: place, date, current condition.
    text(
        target,
        &title,
        &forecast.location_name,
        Point::new(16, 12),
        VerticalPosition::Top,
        HorizontalAlignment::Left,
    )?;

    if let Some(today) = forecast.days.first() {
        let d = today.date;
        let date = format!(
            "{}, {} {} {}",
            d.weekday_short(lang),
            d.day,
            d.month_short(lang),
            d.year
        );
        text(
            target,
            &body,
            &date,
            Point::new(16, 48),
            VerticalPosition::Top,
            HorizontalAlignment::Left,
        )?;
    }
    let now = format!(
        "{} {}",
        lang.now_prefix(),
        forecast.current.condition.label(lang)
    );
    text(
        target,
        &body,
        &now,
        Point::new(16, 76),
        VerticalPosition::Top,
        HorizontalAlignment::Left,
    )?;

    // Right side: current temperature (right-aligned) and a large icon to its left.
    let temp = format!(
        "{:.0}{}",
        forecast.current.temperature, forecast.temperature_symbol
    );
    text(
        target,
        &big,
        &temp,
        Point::new(width - 16, 14),
        VerticalPosition::Top,
        HorizontalAlignment::Right,
    )?;
    icons::draw_icon(
        target,
        forecast.current.condition,
        Point::new(width - 220, 56),
        84,
    )?;

    let refreshed = format!("{} {}", lang.refreshed_prefix(), forecast.refreshed_at);
    text(
        target,
        &small,
        &refreshed,
        Point::new(width - 16, 96),
        VerticalPosition::Top,
        HorizontalAlignment::Right,
    )?;

    Ok(())
}

fn draw_day_cards<D>(target: &mut D, days: &[Day], size: Size, lang: Language) -> Result<()>
where
    D: DrawTarget<Color = BinaryColor>,
    D::Error: core::fmt::Debug,
{
    if days.is_empty() {
        return Ok(());
    }
    let width = size.width as i32;
    let height = size.height as i32;
    let n = days.len() as i32;
    let col_w = width / n;

    let weekday = FontRenderer::new::<fonts::u8g2_font_helvB14_tf>();
    let temps = FontRenderer::new::<fonts::u8g2_font_helvR14_tf>();
    let small = FontRenderer::new::<fonts::u8g2_font_helvR10_tf>();

    let top = HEADER_HEIGHT + 1;
    let card_h = height - top;
    let icon_size = (col_w as f32 * 0.5) as u32;

    for (i, day) in days.iter().enumerate() {
        let i = i as i32;
        let x0 = i * col_w;
        let cx = x0 + col_w / 2;

        if i > 0 {
            vline(target, x0, top + 10, height - 10)?;
        }

        let label = format!("{} {}", day.date.weekday_short(lang), day.date.day);
        text(
            target,
            &weekday,
            &label,
            Point::new(cx, top + 14),
            VerticalPosition::Top,
            HorizontalAlignment::Center,
        )?;

        let icon_cy = top + (card_h as f32 * 0.42) as i32;
        icons::draw_icon(target, day.condition, Point::new(cx, icon_cy), icon_size)?;

        let hi_lo = format!("{:.0}° / {:.0}°", day.temp_max, day.temp_min);
        text(
            target,
            &temps,
            &hi_lo,
            Point::new(cx, top + (card_h as f32 * 0.72) as i32),
            VerticalPosition::Top,
            HorizontalAlignment::Center,
        )?;

        if let Some(p) = day.precip_prob {
            let rain = format!("{p}%");
            text(
                target,
                &small,
                &rain,
                Point::new(cx, top + (card_h as f32 * 0.85) as i32),
                VerticalPosition::Top,
                HorizontalAlignment::Center,
            )?;
        }
    }
    Ok(())
}

fn text<D>(
    target: &mut D,
    font: &FontRenderer,
    s: &str,
    pos: Point,
    vertical: VerticalPosition,
    horizontal: HorizontalAlignment,
) -> Result<()>
where
    D: DrawTarget<Color = BinaryColor>,
    D::Error: core::fmt::Debug,
{
    font.render_aligned(
        s,
        pos,
        vertical,
        horizontal,
        FontColor::Transparent(BLACK),
        target,
    )
    .map_err(|e| anyhow!("font render error: {e:?}"))?;
    Ok(())
}

fn hline<D>(target: &mut D, x0: i32, x1: i32, y: i32) -> Result<()>
where
    D: DrawTarget<Color = BinaryColor>,
    D::Error: core::fmt::Debug,
{
    Line::new(Point::new(x0, y), Point::new(x1, y))
        .into_styled(PrimitiveStyle::with_stroke(BLACK, 2))
        .draw(target)
        .map_err(|e| anyhow!("line draw error: {e:?}"))?;
    Ok(())
}

fn vline<D>(target: &mut D, x: i32, y0: i32, y1: i32) -> Result<()>
where
    D: DrawTarget<Color = BinaryColor>,
    D::Error: core::fmt::Debug,
{
    Line::new(Point::new(x, y0), Point::new(x, y1))
        .into_styled(PrimitiveStyle::with_stroke(BLACK, 1))
        .draw(target)
        .map_err(|e| anyhow!("line draw error: {e:?}"))?;
    Ok(())
}
