//! Margin fills: owned syntax, linear-gradient geometry and deterministic sRGB sampling, and
//! image backdrops (see [`image_fill`]).
//!
//! Fill colors carry straight alpha. Opaque stop pairs interpolate in sRGB; pairs with alpha
//! interpolate premultiplied so a fade to clear does not darken toward the clear stop's color.

use std::path::Path;

use super::canvas::{rgb, Canvas};
use crate::Result;

/// A margin fill spans the final output canvas rather than just the window bounds.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum MarginFill {
    Solid(FillColor),
    Gradient(LinearGradient),
}

/// A straight-alpha fill color. `transparent` is fully clear black.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct FillColor {
    pub(super) color: libghostty_vt::style::RgbColor,
    pub(super) alpha: u8,
}

impl FillColor {
    pub(super) const CLEAR: Self = Self {
        color: rgb(0, 0, 0),
        alpha: 0,
    };

    pub(super) const fn opaque(color: libghostty_vt::style::RgbColor) -> Self {
        Self {
            color,
            alpha: u8::MAX,
        }
    }
}

/// A linear gradient resolved into straight-alpha stops and output-relative geometry.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct LinearGradient {
    direction: GradientDirection,
    stops: Vec<GradientStop>,
}

impl LinearGradient {
    /// Sample the output-relative gradient with a fixed spatial dither.
    pub(super) fn pixel_color(&self, x: u32, y: u32, width: u32, height: u32) -> FillColor {
        let fraction = self.direction.fraction(x, y, width, height);
        let dither =
            (f64::from(GRADIENT_DITHER[y as usize % 4][x as usize % 4]) + 0.5) / 16.0 - 0.5;
        self.color_at(fraction, dither)
    }

    fn from_stops(direction: GradientDirection, stops: Vec<GradientStopInput>) -> Option<Self> {
        if stops.len() < 2 {
            return None;
        }

        let positions_given = stops.iter().any(|stop| stop.position.is_some());
        if positions_given && stops.iter().any(|stop| stop.position.is_none()) {
            return None;
        }

        let last_index = stops.len() - 1;
        let mut previous_position = 0.0;
        let mut resolved = Vec::with_capacity(stops.len());
        for (index, stop) in stops.into_iter().enumerate() {
            let position = stop.position.unwrap_or(index as f64 / last_index as f64);
            if !(0.0..=1.0).contains(&position) || (index > 0 && position <= previous_position) {
                return None;
            }
            previous_position = position;
            resolved.push(GradientStop {
                color: stop.color,
                position,
            });
        }

        Some(Self {
            direction,
            stops: resolved,
        })
    }

    /// Return the dithered sRGB color at one normalized gradient position.
    pub(super) fn color_at(&self, fraction: f64, dither: f64) -> FillColor {
        let fraction = fraction.clamp(0.0, 1.0);
        let first = self.stops.first().expect("gradients always have stops");
        let last = self.stops.last().expect("gradients always have stops");
        if fraction <= first.position {
            return dither_color(first.color, first.color, 0.0, dither);
        }
        if fraction >= last.position {
            return dither_color(last.color, last.color, 0.0, dither);
        }

        let pair = self
            .stops
            .windows(2)
            .find(|pair| fraction <= pair[1].position)
            .expect("gradient positions cover the normalized range");
        let start = pair[0];
        let end = pair[1];
        let span = end.position - start.position;
        let local_fraction = if span == 0.0 {
            1.0
        } else {
            (fraction - start.position) / span
        };
        dither_color(start.color, end.color, local_fraction, dither)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct GradientStop {
    color: FillColor,
    position: f64,
}

#[derive(Debug, Clone, Copy)]
struct GradientStopInput {
    color: FillColor,
    position: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum GradientDirection {
    /// The default runs from the top edge to the bottom edge.
    Vertical,
    /// Degrees clockwise from upward, normalized while parsing.
    Angle(f64),
}

impl GradientDirection {
    /// Project onto the line between the direction's endpoints, including both end pixels.
    fn fraction(self, x: u32, y: u32, width: u32, height: u32) -> f64 {
        let dx = f64::from(width.saturating_sub(1));
        let dy = f64::from(height.saturating_sub(1));
        match self {
            Self::Vertical => f64::from(y) / dy.max(1.0),
            Self::Angle(degrees) => {
                let radians = degrees.to_radians();
                let unit_x = clamp_near_zero(radians.sin());
                let unit_y = clamp_near_zero(-radians.cos());
                let position = f64::from(x) * unit_x + f64::from(y) * unit_y;
                let corners = [0.0, dx * unit_x, dy * unit_y, dx * unit_x + dy * unit_y];
                let min = corners.iter().copied().fold(f64::INFINITY, f64::min);
                let max = corners.iter().copied().fold(f64::NEG_INFINITY, f64::max);
                (position - min) / (max - min).max(1.0)
            }
        }
    }
}

/// Ordered thresholds soften gradient banding without temporal noise or changes to solid fills.
const GRADIENT_DITHER: [[u8; 4]; 4] =
    [[0, 8, 2, 10], [12, 4, 14, 6], [3, 11, 1, 9], [15, 7, 13, 5]];

/// Keep cardinal angles exactly cardinal despite floating-point trigonometry.
fn clamp_near_zero(value: f64) -> f64 {
    if value.abs() < 1e-12 {
        0.0
    } else {
        value
    }
}

/// Blend one stop pair and apply the fixed ordered dither.
fn dither_color(start: FillColor, end: FillColor, fraction: f64, dither: f64) -> FillColor {
    if start.alpha == u8::MAX && end.alpha == u8::MAX {
        return FillColor::opaque(dither_opaque(start.color, end.color, fraction, dither));
    }
    let start_alpha = f64::from(start.alpha);
    let end_alpha = f64::from(end.alpha);
    let alpha = start_alpha + (end_alpha - start_alpha) * fraction;
    let dithered_alpha = (alpha + dither).round().clamp(0.0, 255.0) as u8;
    if dithered_alpha == 0 || alpha <= 0.0 {
        return FillColor::CLEAR;
    }
    let interpolate = |a: u8, b: u8| {
        let start = f64::from(a) * start_alpha;
        let premultiplied = start + (f64::from(b) * end_alpha - start) * fraction;
        (premultiplied / alpha + dither).round().clamp(0.0, 255.0) as u8
    };
    FillColor {
        color: rgb(
            interpolate(start.color.r, end.color.r),
            interpolate(start.color.g, end.color.g),
            interpolate(start.color.b, end.color.b),
        ),
        alpha: dithered_alpha,
    }
}

/// Blend one opaque stop pair in sRGB and apply the fixed ordered dither.
fn dither_opaque(
    start: libghostty_vt::style::RgbColor,
    end: libghostty_vt::style::RgbColor,
    fraction: f64,
    dither: f64,
) -> libghostty_vt::style::RgbColor {
    let interpolate = |a, b| {
        (f64::from(a) + (f64::from(b) - f64::from(a)) * fraction + dither)
            .round()
            .clamp(0.0, 255.0) as u8
    };
    rgb(
        interpolate(start.r, end.r),
        interpolate(start.g, end.g),
        interpolate(start.b, end.b),
    )
}

/// Invalid fills retain the legacy fallback to the theme background.
pub(super) fn parse_margin_fill(value: &str) -> Option<MarginFill> {
    let value = value.trim();
    if let Some(value) = value.strip_prefix("linear(") {
        return parse_linear_gradient(value.strip_suffix(')')?);
    }

    parse_fill_color(value).map(MarginFill::Solid)
}

/// Parse Betamax's comma-separated `linear(...)` margin-fill syntax.
fn parse_linear_gradient(value: &str) -> Option<MarginFill> {
    let arguments: Vec<_> = value.split(',').map(str::trim).collect();
    if arguments.iter().any(|argument| argument.is_empty()) {
        return None;
    }

    let (direction, stops) = match arguments
        .first()
        .and_then(|value| parse_gradient_direction(value))
    {
        Some(direction) => (direction, &arguments[1..]),
        None if arguments.first()?.trim_start().starts_with("angle:") => return None,
        None => (GradientDirection::Vertical, &arguments[..]),
    };
    let stops = stops
        .iter()
        .map(|value| parse_gradient_stop(value))
        .collect::<Option<Vec<_>>>()?;
    LinearGradient::from_stops(direction, stops).map(MarginFill::Gradient)
}

fn parse_gradient_direction(value: &str) -> Option<GradientDirection> {
    let value = value.trim();
    if let Some(destination) = value.strip_prefix("to:") {
        let degrees = match destination.trim() {
            destination if destination.eq_ignore_ascii_case("top") => 0.0,
            destination if destination.eq_ignore_ascii_case("top-right") => 45.0,
            destination if destination.eq_ignore_ascii_case("right") => 90.0,
            destination if destination.eq_ignore_ascii_case("bottom-right") => 135.0,
            destination if destination.eq_ignore_ascii_case("bottom") => 180.0,
            destination if destination.eq_ignore_ascii_case("bottom-left") => 225.0,
            destination if destination.eq_ignore_ascii_case("left") => 270.0,
            destination if destination.eq_ignore_ascii_case("top-left") => 315.0,
            _ => return None,
        };
        return Some(GradientDirection::Angle(degrees));
    }

    let angle = value.strip_prefix("angle:")?.trim();
    let degrees = angle.strip_suffix("deg")?.trim().parse::<f64>().ok()?;
    degrees
        .is_finite()
        .then(|| GradientDirection::Angle(degrees.rem_euclid(360.0)))
}

fn parse_gradient_stop(value: &str) -> Option<GradientStopInput> {
    let mut tokens = value.split_whitespace();
    let color = parse_fill_color(tokens.next()?)?;
    let position = match tokens.next() {
        None => None,
        Some(value) => Some(parse_gradient_position(value)?),
    };
    tokens
        .next()
        .is_none()
        .then_some(GradientStopInput { color, position })
}

fn parse_gradient_position(value: &str) -> Option<f64> {
    let percent = value.strip_suffix('%')?.parse::<f64>().ok()?;
    (percent.is_finite() && (0.0..=100.0).contains(&percent)).then_some(percent / 100.0)
}

/// Prepare the full-output backdrop once, including the area behind captions and corners.
///
/// Image fills resolve relative paths against the working directory, then `tape_dir`. Missing or
/// undecodable images are errors; invalid colors and gradients fall back to the theme background.
pub(super) fn margin_backdrop(
    width: u32,
    height: u32,
    value: Option<&str>,
    fallback: libghostty_vt::style::RgbColor,
    tape_dir: Option<&Path>,
) -> Result<Canvas> {
    if let Some(image) = value.map(MarginImage::parse).transpose()?.flatten() {
        return image.backdrop(width, height, tape_dir, fallback);
    }
    let fill = value
        .and_then(parse_margin_fill)
        .unwrap_or(MarginFill::Solid(FillColor::opaque(fallback)));
    let gradient = match fill {
        MarginFill::Solid(FillColor { color, alpha }) => {
            let mut frame = Canvas::new(width, height, color)?;
            if alpha < u8::MAX {
                frame.fill_alpha(color, alpha);
            }
            return Ok(frame);
        }
        MarginFill::Gradient(gradient) => gradient,
    };
    let mut frame = Canvas::transparent(width, height)?;
    for y in 0..height {
        for x in 0..width {
            let FillColor { color, alpha } = gradient.pixel_color(x, y, width, height);
            frame.set_pixel_alpha(x, y, color, alpha);
        }
    }
    Ok(frame)
}

/// Parse a margin-fill color: `#RRGGBB`, CSS-ordered `#RRGGBBAA`, or `transparent`.
fn parse_fill_color(value: &str) -> Option<FillColor> {
    let value = value.trim();
    if value.eq_ignore_ascii_case("transparent") {
        return Some(FillColor::CLEAR);
    }
    if value.len() == 9 && value.is_ascii() {
        let alpha = u8::from_str_radix(&value[7..], 16).ok()?;
        let color = parse_hex_color(&value[..7])?;
        return Some(if alpha == 0 {
            FillColor::CLEAR
        } else {
            FillColor { color, alpha }
        });
    }
    parse_hex_color(value).map(FillColor::opaque)
}

/// Parse a `#RRGGBB` color.
///
/// Decoration colors are parsed leniently. Invalid values return `None` so callers can fall back
/// to the active theme background rather than failing old tapes that used unsupported color text.
pub(super) fn parse_hex_color(value: &str) -> Option<libghostty_vt::style::RgbColor> {
    let value = value.trim().strip_prefix('#')?;
    if value.len() != 6 || !value.is_ascii() {
        return None;
    }
    Some(rgb(
        u8::from_str_radix(&value[0..2], 16).ok()?,
        u8::from_str_radix(&value[2..4], 16).ok()?,
        u8::from_str_radix(&value[4..6], 16).ok()?,
    ))
}

mod image_fill;
#[cfg(test)]
mod tests;

use image_fill::MarginImage;
