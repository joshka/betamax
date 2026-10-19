use super::*;
use crate::media::Frame;
use crate::runner::settings::Settings;
use crate::tape::Tape;

fn decorated(settings: &Settings, caption: Option<&str>) -> Frame {
    let terminal = Canvas::new(
        settings.presentation.terminal_canvas_width(),
        settings.presentation.terminal_canvas_height(),
        rgb(255, 0, 0),
    )
    .unwrap()
    .into_frame();
    settings
        .presentation
        .decorate_frame_with_overlays(&terminal, caption, &[])
        .unwrap()
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
        Some(MarginFill::Solid(FillColor::opaque(rgb(18, 58, 188))))
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
        let tape =
            Tape::parse(&format!("Set Preset Default-0.1\nSet MarginFill '{value}'")).unwrap();
        let settings = Settings::from_tape(&tape).unwrap();
        let gradient = parsed_gradient(settings.presentation.style.margin_fill.as_deref().unwrap());
        assert_eq!(gradient.direction, GradientDirection::Angle(direction));
        assert_eq!(gradient.stops.len(), 2);
        assert_eq!(
            gradient.stops[0],
            GradientStop {
                color: FillColor::opaque(rgb(35, 38, 64)),
                position: 0.0
            }
        );
        assert_eq!(
            gradient.stops[1],
            GradientStop {
                color: FillColor::opaque(rgb(15, 16, 26)),
                position: 1.0
            }
        );
    }
}

#[test]
fn linear_margin_fill_supports_angles_and_positioned_stops() {
    let gradient = parsed_gradient("linear(angle: 495deg, #4d5b9e 0%, #232640 42%, #0f101a 100%)");
    assert_eq!(gradient.direction, GradientDirection::Angle(135.0));
    assert_eq!(
        gradient.stops,
        vec![
            GradientStop {
                color: FillColor::opaque(rgb(77, 91, 158)),
                position: 0.0
            },
            GradientStop {
                color: FillColor::opaque(rgb(35, 38, 64)),
                position: 0.42
            },
            GradientStop {
                color: FillColor::opaque(rgb(15, 16, 26)),
                position: 1.0
            },
        ]
    );
    assert_eq!(gradient.color_at(0.42, 0.0).color, rgb(35, 38, 64));
    assert_eq!(gradient.color_at(0.71, 0.0).color, rgb(25, 27, 45));
}

#[test]
fn linear_margin_fill_uses_its_angle_as_the_first_to_last_stop_direction() {
    let rightward = margin_backdrop(
        3,
        3,
        Some("linear(angle: 90deg, #000000, #ffffff)"),
        rgb(255, 0, 0),
    )
    .unwrap()
    .into_frame();
    assert_eq!(pixel(&rightward, 0, 0), [0, 0, 0, 255]);
    assert_eq!(pixel(&rightward, 2, 2), [255, 255, 255, 255]);

    let upward = margin_backdrop(
        3,
        3,
        Some("linear(angle: 0deg, #000000, #ffffff)"),
        rgb(255, 0, 0),
    )
    .unwrap()
    .into_frame();
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
            "Set Preset Default-0.1\nSet Width 12\nSet Height 12\nSet Margin 2\nSet MarginFill '{value}'\nSet Theme '{{\"background\":\"#123456\"}}'"
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
        let tape = Tape::parse(&format!("Set Preset Default-0.1\nSet MarginFill {value}")).unwrap();
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
            "Set Preset Default-0.1\nSet Width 5\nSet Height 5\nSet Margin 1\n{source}"
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
            "Set Preset Default-0.1\nSet Width 5\nSet Height 9\nSet Margin 2\nSet MarginFill 'linear({direction}, #000000, #ffffff)'"
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
        let frame = margin_backdrop(
            width,
            height,
            Some(&format!("linear({direction}, #000000, #ffffff)")),
            rgb(255, 0, 0),
        )
        .unwrap()
        .into_frame();
        assert_eq!(
            pixel(&frame, width - 1, height - 1),
            [last, last, last, 255]
        );
    }
}

#[test]
fn gradient_dither_is_fixed_and_preserves_endpoints() {
    let tape = Tape::parse(
        "Set Preset Default-0.1\nSet Width 8\nSet Height 3\nSet Margin 1\nSet MarginFill 'linear(#000000, #ffffff)'",
    )
    .unwrap();
    let settings = Settings::from_tape(&tape).unwrap();
    let backdrop = margin_backdrop(
        settings.presentation.width,
        settings.presentation.height,
        settings.presentation.style.margin_fill.as_deref(),
        settings.presentation.theme.background,
    )
    .unwrap()
    .into_frame();
    let frame = &backdrop;
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
        Set Preset Default-0.1
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
    // Continuous corners mix the red terminal with the same-position grayscale gradient.
    let actual = pixel(&frame, 3, 12);
    assert!(actual[0] < 255 && actual[0] > actual[1], "{actual:?}");
    assert!(actual[1] > 0);
    assert_eq!(actual[1], actual[2]);
    assert_eq!(actual[3], 255);
    assert_eq!(pixel(&frame, 7, 7), [255, 0, 0, 255]);
}

#[test]
fn unset_margin_fill_uses_the_final_theme_background() {
    let tape = Tape::parse(
        r##"
        Set Preset Default-0.1
        Set Width 5
        Set Height 5
        Set Margin 1
        Set Theme '{"background":"#123456"}'
    "##,
    )
    .unwrap();
    let settings = Settings::from_tape(&tape).unwrap();
    assert_eq!(settings.presentation.style.margin_fill, None);
    assert_eq!(pixel(&decorated(&settings, None), 0, 0), [18, 52, 86, 255]);
}

#[test]
fn default_theme_and_single_color_margin_pixels_keep_expected_bytes() {
    for (setting, fill) in [
        ("", [16, 32, 64, 255]),
        ("Set MarginFill '#123456'", [18, 52, 86, 255]),
    ] {
        let tape = Tape::parse(&format!(
            "Set Preset Default-0.1\nSet Width 5\nSet Height 5\nSet Margin 1\n{setting}"
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

fn pixel(frame: &Frame, x: u32, y: u32) -> [u8; 4] {
    let offset = ((y * frame.width + x) * 4) as usize;
    [
        frame.pixels[offset],
        frame.pixels[offset + 1],
        frame.pixels[offset + 2],
        frame.pixels[offset + 3],
    ]
}

#[test]
fn transparent_margin_fill_clears_the_backdrop_around_an_opaque_window() {
    assert_eq!(
        parse_margin_fill(" Transparent "),
        Some(MarginFill::Solid(FillColor::CLEAR))
    );
    let tape =
        Tape::parse("Set Preset Framed\nSet Width 200\nSet Height 160\nSet MarginFill transparent")
            .unwrap();
    let settings = Settings::from_tape(&tape).unwrap();
    assert!(settings.presentation.style.margin_has_alpha());
    let frame = decorated(&settings, None);
    let alpha =
        |x: u32, y: u32| frame.pixels[(y as usize * frame.width as usize + x as usize) * 4 + 3];

    assert_eq!(alpha(0, 0), 0, "margin is clear");
    assert_eq!(alpha(100, 80), 255, "terminal stays opaque");
    let below_window = alpha(100, 160 - 32 + 6);
    assert!(
        (1..255).contains(&below_window),
        "shadow is translucent: {below_window}"
    );
}

#[test]
fn coverage_against_a_clear_backdrop_fades_alpha_not_color() {
    let mut clear = Canvas::transparent(1, 1).unwrap();
    let mut window = Canvas::new(1, 1, rgb(200, 100, 50)).unwrap();
    window.composite_pixel(0, 0, rgb(0, 0, 0), 0.0, &clear, 0.25);
    assert_eq!(window.clone().into_frame().pixels, vec![200, 100, 50, 191]);

    // A translucent shadow sample contributes its color by its own coverage.
    clear.blend_pixel(0, 0, 0, 0, 0, 128);
    let mut window = Canvas::new(1, 1, rgb(200, 100, 50)).unwrap();
    window.composite_pixel(0, 0, rgb(0, 0, 0), 0.0, &clear, 0.5);
    assert_eq!(window.into_frame().pixels, vec![133, 67, 33, 192]);
}

#[test]
fn opaque_margin_fills_keep_every_pixel_opaque() {
    let tape = Tape::parse("Set Preset Framed\nSet Width 200\nSet Height 160").unwrap();
    let settings = Settings::from_tape(&tape).unwrap();
    assert!(!settings.presentation.style.margin_has_alpha());
    let frame = decorated(&settings, Some("caption"));
    assert!(frame
        .pixels
        .as_chunks::<4>()
        .0
        .iter()
        .all(|pixel| pixel[3] == 255));
}

#[test]
fn margin_fill_accepts_css_ordered_alpha() {
    assert_eq!(
        parse_margin_fill("#0f101a80"),
        Some(MarginFill::Solid(FillColor {
            color: rgb(15, 16, 26),
            alpha: 128
        }))
    );
    assert_eq!(
        parse_margin_fill("#12345600"),
        Some(MarginFill::Solid(FillColor::CLEAR))
    );
    assert!(parse_margin_fill("#0f101aff").unwrap().is_opaque());
    for invalid in ["#0f101a8", "#0f101agg", "#0f101a800"] {
        assert_eq!(parse_margin_fill(invalid), None, "{invalid}");
    }
    let fade = parsed_gradient("linear(#4d5b9e, transparent)");
    assert!(!MarginFill::Gradient(fade).is_opaque());
}

#[test]
fn alpha_gradients_interpolate_premultiplied() {
    let fade = parsed_gradient("linear(#ff0000ff, transparent)");
    // Premultiplied blending keeps the hue while coverage falls, instead of darkening toward black.
    assert_eq!(
        fade.color_at(0.5, 0.0),
        FillColor {
            color: rgb(255, 0, 0),
            alpha: 128
        }
    );
    assert_eq!(fade.color_at(1.0, 0.0), FillColor::CLEAR);
    let mixed = parsed_gradient("linear(#ff000080, #0000ff80)");
    assert_eq!(
        mixed.color_at(0.5, 0.0),
        FillColor {
            color: rgb(128, 0, 128),
            alpha: 128
        }
    );
}

#[test]
fn translucent_solid_fill_keeps_its_alpha_in_the_backdrop() {
    let tape = Tape::parse(
        "Set Preset Framed\nSet Width 200\nSet Height 160\nSet Shadow false\nSet MarginFill \"#0f101a80\"",
    )
    .unwrap();
    let settings = Settings::from_tape(&tape).unwrap();
    assert!(settings.presentation.style.margin_has_alpha());
    let frame = decorated(&settings, None);
    assert_eq!(frame.pixels[..4], [15, 16, 26, 128]);
}
