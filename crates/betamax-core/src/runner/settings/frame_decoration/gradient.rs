//! Linear margin fills: owned syntax, geometry, stops, and deterministic sRGB sampling.

use super::{parse_hex_color, rgb};

/// A margin fill spans the final output canvas rather than just the window bounds.
#[derive(Debug, Clone, PartialEq)]
pub(super) enum MarginFill {
    Solid(libghostty_vt::style::RgbColor),
    Gradient(LinearGradient),
}

/// A linear gradient resolved into opaque RGB stops and output-relative geometry.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct LinearGradient {
    direction: GradientDirection,
    stops: Vec<GradientStop>,
}

impl LinearGradient {
    /// Sample the output-relative gradient with a fixed spatial dither.
    pub(super) fn pixel_color(
        &self,
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    ) -> libghostty_vt::style::RgbColor {
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
    pub(super) fn color_at(&self, fraction: f64, dither: f64) -> libghostty_vt::style::RgbColor {
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
    color: libghostty_vt::style::RgbColor,
    position: f64,
}

#[derive(Debug, Clone, Copy)]
struct GradientStopInput {
    color: libghostty_vt::style::RgbColor,
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

/// Blend one stop pair in sRGB and apply the fixed ordered dither.
fn dither_color(
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

    parse_hex_color(value).map(MarginFill::Solid)
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
    let color = parse_hex_color(tokens.next()?)?;
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

#[cfg(test)]
mod tests {
    use super::super::super::{Settings, Tape};
    use super::super::SolidFrame;
    use super::*;

    fn decorated(settings: &Settings, caption: Option<&str>) -> SolidFrame {
        let terminal = SolidFrame::new(
            settings.terminal_canvas_width(),
            settings.terminal_canvas_height(),
            rgb(255, 0, 0),
        )
        .unwrap()
        .into_frame();
        let frame = settings
            .decorate_frame_with_overlays(&terminal, caption, &[])
            .unwrap();
        SolidFrame {
            width: frame.width,
            height: frame.height,
            pixels: frame.pixels,
        }
    }

    fn parsed_gradient(value: &str) -> LinearGradient {
        match parse_margin_fill(value) {
            Some(MarginFill::Gradient(gradient)) => gradient,
            _ => panic!("expected a gradient for {value}"),
        }
    }

    #[test]
    fn margin_fill_parses_colors_and_gradient_destinations() {
        assert_eq!(
            parse_margin_fill(" #123aBc "),
            Some(MarginFill::Solid(rgb(18, 58, 188)))
        );
        assert_eq!(
            parsed_gradient("linear(#232640, #0f101a)").direction,
            GradientDirection::Vertical
        );
        for (destination, direction) in [
            ("top", 0.0),
            ("top-right", 45.0),
            ("right", 90.0),
            ("bottom-right", 135.0),
            ("bottom", 180.0),
            ("bottom-left", 225.0),
            ("left", 270.0),
            ("top-left", 315.0),
        ] {
            let value = format!("linear(to: {destination}, #232640, #0f101a)");
            let tape = Tape::parse(&format!("Set MarginFill '{value}'")).unwrap();
            let settings = Settings::from_tape(&tape).unwrap();
            let gradient = parsed_gradient(settings.style.margin_fill.as_deref().unwrap());
            assert_eq!(gradient.direction, GradientDirection::Angle(direction));
            assert_eq!(gradient.stops.len(), 2);
            assert_eq!(
                gradient.stops[0],
                GradientStop {
                    color: rgb(35, 38, 64),
                    position: 0.0
                }
            );
            assert_eq!(
                gradient.stops[1],
                GradientStop {
                    color: rgb(15, 16, 26),
                    position: 1.0
                }
            );
        }
    }

    #[test]
    fn linear_margin_fill_supports_angles_and_positioned_stops() {
        let gradient =
            parsed_gradient("linear(angle: 495deg, #4d5b9e 0%, #232640 42%, #0f101a 100%)");
        assert_eq!(gradient.direction, GradientDirection::Angle(135.0));
        assert_eq!(
            gradient.stops,
            vec![
                GradientStop {
                    color: rgb(77, 91, 158),
                    position: 0.0
                },
                GradientStop {
                    color: rgb(35, 38, 64),
                    position: 0.42
                },
                GradientStop {
                    color: rgb(15, 16, 26),
                    position: 1.0
                },
            ]
        );
        assert_eq!(gradient.color_at(0.42, 0.0), rgb(35, 38, 64));
        assert_eq!(gradient.color_at(0.71, 0.0), rgb(25, 27, 45));
    }

    #[test]
    fn linear_margin_fill_uses_its_angle_as_the_first_to_last_stop_direction() {
        let rightward = SolidFrame::margin_backdrop(
            3,
            3,
            Some("linear(angle: 90deg, #000000, #ffffff)"),
            rgb(255, 0, 0),
        )
        .unwrap();
        assert_eq!(pixel(&rightward, 0, 0), [0, 0, 0, 255]);
        assert_eq!(pixel(&rightward, 2, 2), [255, 255, 255, 255]);

        let upward = SolidFrame::margin_backdrop(
            3,
            3,
            Some("linear(angle: 0deg, #000000, #ffffff)"),
            rgb(255, 0, 0),
        )
        .unwrap();
        assert_eq!(pixel(&upward, 0, 2), [0, 0, 0, 255]);
        assert_eq!(pixel(&upward, 2, 0), [255, 255, 255, 255]);
    }

    #[test]
    fn invalid_margin_fills_fall_back_to_the_final_theme_background() {
        for value in [
            "red",
            "#12345",
            "#12é34",
            "#1é234",
            "#000000 #ffffff",
            "#000000 #gggggg",
            "#000000 vertical",
            "linear()",
            "linear(to: middle, #000000, #ffffff)",
            "linear(angle: 135deg, #000000)",
            "linear(angle: 135degrees, #000000, #ffffff)",
            "linear(angle: 135deg, #000000 0%, #ffffff)",
            "linear(angle: 135deg, #000000 60%, #ffffff 40%)",
            "linear(angle: 135deg, #000000 0%, #ffffff 0%)",
        ] {
            assert!(parse_margin_fill(value).is_none(), "{value}");
            let tape = Tape::parse(&format!(
                "Set Width 12\nSet Height 12\nSet Margin 2\nSet MarginFill '{value}'\nSet Theme '{{\"background\":\"#123456\"}}'"
            )).unwrap();
            let settings = Settings::from_tape(&tape).unwrap();
            assert_eq!(
                pixel(&decorated(&settings, None), 0, 0),
                [18, 52, 86, 255],
                "{value}"
            );
        }
    }

    #[test]
    fn margin_fill_wrong_types_use_the_existing_setting_error() {
        for (value, kind) in [("true", "bool"), ("123", "number"), ("1s", "duration")] {
            let tape = Tape::parse(&format!("Set MarginFill {value}")).unwrap();
            let error = Settings::from_tape(&tape).unwrap_err().to_string();
            assert!(
                error.contains(&format!("Set MarginFill expects string, got {kind}")),
                "{error}"
            );
        }
    }

    #[test]
    fn explicit_margin_gradients_survive_theme_ordering() {
        let theme = r##"Set Theme '{"background":"#123456"}'"##;
        let fill = r##"Set MarginFill "linear(#000000, #ffffff)""##;
        for source in [format!("{theme}\n{fill}"), format!("{fill}\n{theme}")] {
            let tape = Tape::parse(&format!(
                "Set Width 5\nSet Height 5\nSet Margin 1\n{source}"
            ))
            .unwrap();
            let settings = Settings::from_tape(&tape).unwrap();
            let frame = decorated(&settings, None);
            assert_eq!(pixel(&frame, 0, 0), [0, 0, 0, 255]);
            assert_eq!(pixel(&frame, 0, 2), [127, 127, 127, 255]);
            assert_eq!(pixel(&frame, 4, 4), [255, 255, 255, 255]);
        }
    }

    #[test]
    fn gradients_span_the_output_in_all_directions() {
        for (direction, midpoint) in [
            ("to: bottom", 32),
            ("to: right", 128),
            ("to: bottom-right", 64),
        ] {
            let tape = Tape::parse(&format!(
                "Set Width 5\nSet Height 9\nSet Margin 2\nSet MarginFill 'linear({direction}, #000000, #ffffff)'"
            )).unwrap();
            let settings = Settings::from_tape(&tape).unwrap();
            let frame = decorated(&settings, None);
            assert_eq!(pixel(&frame, 0, 0), [0, 0, 0, 255]);
            assert_eq!(pixel(&frame, 4, 8), [255, 255, 255, 255]);
            assert_eq!(pixel(&frame, 2, 2), [255, 0, 0, 255]);
            assert_eq!(pixel(&frame, 2, 1), [midpoint, midpoint, midpoint, 255]);
        }
    }

    #[test]
    fn gradient_endpoints_handle_single_pixel_axes() {
        for (width, height, direction, last) in [
            (1, 1, "to: bottom-right", 0),
            (1, 3, "to: right", 0),
            (3, 1, "to: bottom", 0),
            (1, 3, "to: bottom-right", 255),
            (3, 1, "to: bottom-right", 255),
        ] {
            let frame = SolidFrame::margin_backdrop(
                width,
                height,
                Some(&format!("linear({direction}, #000000, #ffffff)")),
                rgb(255, 0, 0),
            )
            .unwrap();
            assert_eq!(
                pixel(&frame, width - 1, height - 1),
                [last, last, last, 255]
            );
        }
    }

    #[test]
    fn gradient_dither_is_fixed_and_preserves_endpoints() {
        let tape = Tape::parse(
            "Set Width 8\nSet Height 3\nSet Margin 1\nSet MarginFill 'linear(#000000, #ffffff)'",
        )
        .unwrap();
        let settings = Settings::from_tape(&tape).unwrap();
        let frame = &settings.backdrop;
        assert_eq!(pixel(frame, 0, 0), [0, 0, 0, 255]);
        assert_eq!(pixel(frame, 7, 2), [255, 255, 255, 255]);
        assert_eq!(pixel(frame, 0, 1), [128, 128, 128, 255]);
        assert_eq!(pixel(frame, 1, 1), [127, 127, 127, 255]);
        assert_eq!(
            decorated(&settings, None).pixels,
            decorated(&settings, None).pixels
        );
    }

    #[test]
    fn rounded_window_corners_restore_and_blend_the_local_backdrop() {
        let tape = Tape::parse(
            r##"
            Set Width 16
            Set Height 16
            Set Margin 2
            Set MarginFill "linear(to: right, #000000, #ffffff)"
            Set BorderRadius 5
            Set WindowBar Colorful
            Set WindowBarSize 2
        "##,
        )
        .unwrap();
        let settings = Settings::from_tape(&tape).unwrap();
        let frame = decorated(&settings, None);
        assert_eq!(pixel(&frame, 2, 2), [34, 34, 34, 255]);
        assert_eq!(pixel(&frame, 13, 13), [221, 221, 221, 255]);
        // (4, 12) has 5/16 coverage of the red terminal. The backdrop here is #444444.
        let actual = pixel(&frame, 4, 12);
        for (actual, expected) in actual.into_iter().zip([126u8, 46, 46, 255]) {
            assert!(actual.abs_diff(expected) <= 1, "{actual} != {expected}");
        }
        assert_eq!(pixel(&frame, 7, 7), [255, 0, 0, 255]);
    }

    #[test]
    fn caption_row_keeps_the_gradient_behind_its_text() {
        let tape = Tape::parse(
            r##"
            Set Width 320
            Set Height 200
            Set Padding 0
            Set Margin 16
            Set MarginFill "linear(to: right, #000000, #ffffff)"
            Caption "Gradient"
        "##,
        )
        .unwrap();
        let settings = Settings::from_tape(&tape).unwrap();
        let frame = decorated(&settings, Some("Gradient"));
        let y = settings.height - settings.effective_margin() - 1;
        assert_eq!(pixel(&frame, 0, y), [0, 0, 0, 255]);
        assert_eq!(pixel(&frame, 160, y), [128, 128, 128, 255]);
        assert_eq!(pixel(&frame, 319, y), [255, 255, 255, 255]);
        let layout = settings.caption_layout(0).unwrap();
        assert!(
            (layout.text_y..layout.text_y + layout.text_height).any(|y| (layout.text_x..160)
                .any(|x| pixel(&frame, x, y) != pixel(&settings.backdrop, x, y)))
        );
    }

    #[test]
    fn unset_margin_fill_uses_the_final_theme_background() {
        let tape = Tape::parse(
            r##"
            Set Width 5
            Set Height 5
            Set Margin 1
            Set Theme '{"background":"#123456"}'
        "##,
        )
        .unwrap();
        let settings = Settings::from_tape(&tape).unwrap();
        assert_eq!(settings.style.margin_fill, None);
        assert_eq!(pixel(&decorated(&settings, None), 0, 0), [18, 52, 86, 255]);
    }

    #[test]
    fn default_theme_and_single_color_margin_pixels_keep_expected_bytes() {
        for (setting, fill) in [
            ("", [16, 32, 64, 255]),
            ("Set MarginFill '#123456'", [18, 52, 86, 255]),
        ] {
            let tape = Tape::parse(&format!(
                "Set Width 5\nSet Height 5\nSet Margin 1\n{setting}"
            ))
            .unwrap();
            let settings = Settings::from_tape(&tape).unwrap();
            let frame = decorated(&settings, None);
            let expected: Vec<u8> = (0..5)
                .flat_map(|y| {
                    (0..5).flat_map(move |x| {
                        if x == 0 || x == 4 || y == 0 || y == 4 {
                            fill
                        } else {
                            [255, 0, 0, 255]
                        }
                    })
                })
                .collect();
            assert_eq!(frame.pixels, expected);
        }
    }

    fn pixel(frame: &SolidFrame, x: u32, y: u32) -> [u8; 4] {
        let offset = frame.offset(x, y);
        [
            frame.pixels[offset],
            frame.pixels[offset + 1],
            frame.pixels[offset + 2],
            frame.pixels[offset + 3],
        ]
    }
}
