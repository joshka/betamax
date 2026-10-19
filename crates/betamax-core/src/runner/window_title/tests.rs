use std::time::Duration;

use cosmic_text::{Attrs, Family, Metrics};

use super::*;
use crate::runner::canvas::rgb;
use crate::runner::settings::Settings;
use crate::tape::Tape;

fn title_settings(extra: &str) -> Settings {
    let source = format!("Set Width 320\nSet Height 160\nSet Padding 0\nSet Margin 10\nSet WindowBar Colorful\n{extra}");
    let tape = Tape::parse(&format!("Set Preset Default-0.1\n{source}")).unwrap();
    Settings::from_tape(&tape).unwrap()
}

fn title_frame(settings: &Settings) -> Frame {
    let width = settings.presentation.terminal_canvas_width();
    let height = settings.presentation.terminal_canvas_height();
    let raw = Canvas::new(width, height, rgb(1, 2, 3))
        .unwrap()
        .into_frame();
    settings
        .presentation
        .decorate_frame_with_overlays(&raw, None, &[])
        .unwrap()
}

fn frame_pixel(frame: &Frame, x: u32, y: u32) -> [u8; 4] {
    let offset = (y as usize * frame.width as usize + x as usize) * 4;
    frame.pixels[offset..offset + 4].try_into().unwrap()
}

#[test]
fn window_title_settings_accept_strings_and_reject_wrong_types() {
    let settings = title_settings("Set WindowTitle \"jk — log\"\nSet WindowTitleColor \"#707380\"");
    assert_eq!(settings.presentation.style.window_title.text, "jk — log");
    assert_eq!(settings.presentation.style.window_title.color, "#707380");
    for key in ["WindowTitle", "WindowTitleColor"] {
        for value in ["true", "12", "2s"] {
            let tape = Tape::parse(&format!("Set Preset Default-0.1\nSet {key} {value}")).unwrap();
            let error = Settings::from_tape(&tape).unwrap_err().to_string();
            assert!(
                error.contains(&format!("Set {key} expects string")),
                "{error}"
            );
        }
    }
}

#[test]
fn window_title_colors_follow_theme_and_preserve_explicit_overrides() {
    let theme = r##"Set Theme '{"background":"#112233","foreground":"#77a9ed"}'"##;
    let plain = title_settings(&format!("Set WindowTitle MMMM\n{theme}"));
    let invalid = title_settings(&format!(
        "Set WindowTitleColor invalid\nSet WindowTitle MMMM\n{theme}"
    ));
    let automatic = automatic_title_color(rgb(0x77, 0xa9, 0xed), rgb(0x11, 0x22, 0x33));
    let explicit = title_settings(&format!(
        "Set WindowTitle MMMM\n{theme}\nSet WindowTitleColor \"#{:02x}{:02x}{:02x}\"",
        automatic.r, automatic.g, automatic.b
    ));
    let auto = title_settings(&format!(
        "Set WindowTitleColor auto\nSet WindowTitle MMMM\n{theme}"
    ));
    assert_eq!(title_frame(&plain).pixels, title_frame(&auto).pixels);
    assert_eq!(title_frame(&plain).pixels, title_frame(&invalid).pixels);
    assert_eq!(title_frame(&plain).pixels, title_frame(&explicit).pixels);
    let rendered = title_frame(&plain);
    let ink = (10..40).flat_map(|y| (80..240).map(move |x| (x, y)));
    let (x, y) = ink
        .max_by_key(|&(x, y)| frame_pixel(&rendered, x, y)[0])
        .unwrap();
    let sample = frame_pixel(&rendered, x, y);
    // Small normal-weight glyphs can be entirely antialiased. Check their blended hue
    // rather than assuming a pixel has full coverage of the title color.
    assert!(sample[0] > 0x11 + 10);
    let coverage = f32::from(sample[0] - 0x11) / f32::from(automatic.r - 0x11);
    for (actual, background, foreground) in [
        (sample[1], 0x22, automatic.g),
        (sample[2], 0x33, automatic.b),
    ] {
        let expected =
            (background as f32 + coverage * (foreground - background) as f32).round() as i32;
        assert!((i32::from(actual) - expected).abs() <= 1);
    }
    let before = title_settings(&format!(
        "Set WindowTitle MMMM\nSet WindowTitleColor \"#abcdef\"\n{theme}"
    ));
    let after = title_settings(&format!(
        "{theme}\nSet WindowTitleColor \"#abcdef\"\nSet WindowTitle MMMM"
    ));
    assert_eq!(title_frame(&before).pixels, title_frame(&after).pixels);
}

#[test]
fn automatic_color_adapts_to_dark_light_and_overridden_bars() {
    for (foreground, bar, expected) in [
        (rgb(211, 215, 235), rgb(21, 23, 35), rgb(145, 148, 165)),
        (rgb(180, 188, 202), rgb(15, 20, 31), rgb(122, 129, 142)),
        (rgb(76, 79, 105), rgb(239, 241, 245), rgb(105, 108, 130)),
        (rgb(211, 215, 235), rgb(15, 16, 26), rgb(142, 145, 162)),
        (rgb(20, 20, 20), rgb(20, 20, 20), rgb(255, 255, 255)),
        (rgb(240, 240, 240), rgb(240, 240, 240), rgb(0, 0, 0)),
    ] {
        let color = automatic_title_color(foreground, bar);
        assert_eq!(color, expected);
        let ink = relative_luminance(color);
        let background = relative_luminance(bar);
        assert!((ink.max(background) + 0.05) / (ink.min(background) + 0.05) >= 4.5);
    }
    let theme = r##"Set Theme '{"background":"#eff1f5","foreground":"#4c4f69"}'"##;
    let auto = title_settings(&format!(
        "{theme}\nSet WindowBarColor \"#151723\"\nSet WindowTitle MMMM"
    ));
    let color = automatic_title_color(rgb(76, 79, 105), rgb(21, 23, 35));
    let explicit = title_settings(&format!(
        "{theme}\nSet WindowBarColor \"#151723\"\nSet WindowTitle MMMM\nSet WindowTitleColor \"#{:02x}{:02x}{:02x}\"",
        color.r, color.g, color.b
    ));
    assert_eq!(title_frame(&auto).pixels, title_frame(&explicit).pixels);
    // A valid override remains authoritative even when it has no contrast with the bar.
    let hidden = title_settings(&format!(
        "{theme}\nSet WindowTitle MMMM\nSet WindowTitleColor \"#eff1f5\""
    ));
    let plain = title_settings(theme);
    assert_eq!(title_frame(&hidden).pixels, title_frame(&plain).pixels);
}

#[test]
fn window_title_defaults_and_disabled_bar_preserve_pixels_and_grid() {
    let plain = title_settings("");
    let empty = title_settings("Set WindowTitle \"\"\nSet WindowTitleColor \"#ff0000\"");
    assert_eq!(title_frame(&plain).pixels, title_frame(&empty).pixels);
    let disabled = title_settings("Set WindowBar \"\"");
    let ignored = title_settings("Set WindowBar \"\"\nSet WindowTitle \"ignored\"");
    assert_eq!(title_frame(&disabled).pixels, title_frame(&ignored).pixels);
    let titled = title_settings("Set WindowTitle MMMM");
    assert_eq!((plain.columns, plain.rows), (titled.columns, titled.rows));
    assert_eq!(
        plain.presentation.terminal_canvas_height(),
        titled.presentation.terminal_canvas_height()
    );
    // With no decorations, the default output remains the raw terminal pixels.
    let settings = Settings::from_tape(
        &Tape::parse("Set Preset Default-0.1\nSet Width 80\nSet Height 80\nSet Padding 0").unwrap(),
    )
    .unwrap();
    let raw = Canvas::new(80, 80, rgb(1, 2, 3)).unwrap().into_frame();
    assert_eq!(
        settings
            .presentation
            .decorate_frame_with_overlays(&raw, None, &[])
            .unwrap()
            .pixels,
        raw.pixels
    );
}

#[test]
fn window_title_pixels_are_centered_clipped_and_leave_buttons_untouched() {
    for mode in ["Colorful", "ColorfulRight", "Rings", "RingsRight"] {
        for height in [30, 50] {
            let base =
                format!("Set WindowBar {mode}\nSet WindowBarSize {height}\nSet BorderRadius 8");
            let plain = title_frame(&title_settings(&base));
            let titled = title_frame(&title_settings(&format!(
                "{base}\nSet WindowTitle MMMM\nSet WindowTitleColor \"#ffffff\""
            )));
            let mut ink = Vec::new();
            for y in 0..160 {
                for x in 0..320 {
                    assert_eq!(frame_pixel(&titled, x, y)[3], 255);
                    if frame_pixel(&plain, x, y) != frame_pixel(&titled, x, y) {
                        assert!((10..10 + height).contains(&y));
                        ink.push((x, y));
                    }
                }
            }
            let min_x = ink.iter().map(|p| p.0).min().unwrap();
            let max_x = ink.iter().map(|p| p.0).max().unwrap();
            assert!((min_x as i32 + max_x as i32 - 319).abs() <= 3);
            let min_y = ink.iter().map(|p| p.1).min().unwrap();
            let max_y = ink.iter().map(|p| p.1).max().unwrap();
            assert!((min_y as i32 + max_y as i32 - (20 + height - 1) as i32).abs() <= 5);
            // Margin, terminal, top corner, and all button centers retain their colors.
            assert_eq!(frame_pixel(&titled, 0, 0), frame_pixel(&plain, 0, 0));
            assert_eq!(frame_pixel(&titled, 160, 100), [1, 2, 3, 255]);
            assert_eq!(frame_pixel(&titled, 10, 10), frame_pixel(&plain, 10, 10));
            let radius = (height / 6).max(2);
            let first = if mode.ends_with("Right") {
                320 - (10 + height / 2 + radius * 6)
            } else {
                10 + height / 2
            };
            for index in 0..3 {
                let x = first + index * radius * 3;
                assert_eq!(
                    frame_pixel(&titled, x, 10 + height / 2),
                    frame_pixel(&plain, x, 10 + height / 2)
                );
            }
        }
    }
}

#[test]
fn truncated_window_title_leaves_a_visible_gap_from_buttons_with_margin() {
    for mode in ["Colorful", "ColorfulRight", "Rings", "RingsRight"] {
        let base = format!(
            "Set Width 1000\nSet Height 560\nSet Margin 44\nSet Padding 20\nSet BorderRadius 11\nSet WindowBar {mode}"
        );
        let plain = title_frame(&title_settings(&base));
        let titled = title_frame(&title_settings(&format!(
            "{base}\nSet WindowTitle \"{}\"\nSet WindowTitleColor \"#ffffff\"",
            "M".repeat(300)
        )));
        let mut button_columns = Vec::new();
        let mut title_columns = Vec::new();
        for y in 44..74 {
            for x in 0..1000 {
                let before = frame_pixel(&plain, x, y);
                let after = frame_pixel(&titled, x, y);
                if [[255, 95, 87, 255], [255, 189, 46, 255], [40, 200, 64, 255]].contains(&before) {
                    assert_eq!(
                        before, after,
                        "{mode}: title changes a button at ({x}, {y})"
                    );
                    button_columns.push(x);
                }
                if before != after {
                    title_columns.push(x);
                }
            }
        }
        let title_left = *title_columns.iter().min().unwrap();
        let title_right = *title_columns.iter().max().unwrap();
        let gap = if mode.ends_with("Right") {
            button_columns
                .iter()
                .min()
                .unwrap()
                .saturating_sub(title_right + 1)
        } else {
            title_left.saturating_sub(button_columns.iter().max().unwrap() + 1)
        };
        // At a 30 px bar the radius is 5 px. Require two radii of actual blank pixels,
        // a stronger guard than merely keeping text out of the colored button pixels.
        assert!(
            gap >= 10,
            "{mode}: only {gap} blank pixels between title and buttons"
        );
        assert!(
            (title_left as i32 + title_right as i32 - 999).abs() <= 3,
            "{mode}: title is off center"
        );
    }
}

#[test]
fn window_title_pixels_are_reused_between_outputs() {
    let settings = title_settings("Set WindowTitle Cached");
    let title = &settings.presentation.style.window_title;
    assert!(title.pixels.borrow().is_empty());
    let screenshot = title_frame(&settings);
    let cached_pixels = title.pixels.borrow()[0].1.as_ref().unwrap().pixels.as_ptr();
    let width = settings.presentation.terminal_canvas_width();
    let height = settings.presentation.terminal_canvas_height();
    let raw = Canvas::new(width, height, rgb(1, 2, 3))
        .unwrap()
        .into_frame();
    let captured = super::super::capture::CapturedFrame {
        frame: raw,
        caption: None,
        keyboard_overlay_labels: Vec::new(),
        window_title: None,
        title_transition: false,
    };
    let frames = settings
        .presentation
        .decorate_captured_frames([
            (captured.clone(), Duration::from_millis(20)),
            (captured, Duration::from_millis(20)),
        ])
        .unwrap();
    assert_eq!(frames[0].0.pixels, screenshot.pixels);
    assert_eq!(frames[1].0.pixels, screenshot.pixels);
    assert_eq!(
        title.pixels.borrow()[0].1.as_ref().unwrap().pixels.as_ptr(),
        cached_pixels
    );
}

#[test]
fn window_title_truncates_unicode_and_tiny_bars_safely() {
    let mut renderer = CaptionRenderer::new();
    let layout = CaptionLayout {
        text_x: 0,
        text_y: 0,
        text_width: 70,
        text_height: 30,
        font_size: 12.0,
        line_height: 14.4,
    };
    let attrs = Attrs::new().family(Family::Monospace);
    let metrics = Metrics::new(12.0, 14.4);
    let truncated = renderer.truncate_text(
        "日本語 — a very long window title",
        layout,
        metrics,
        &attrs,
        "…",
    );
    assert!(truncated.ends_with('…'));
    assert!(renderer.caption_text_width(&truncated, layout, metrics, &attrs) <= 70.0);
    let long = "日本語 — a very long window title".repeat(10);
    for mode in ["Colorful", "RingsRight"] {
        for width in [40_u32, 113, 120, 200] {
            let base = format!("Set Width {width}\nSet WindowBar {mode}");
            let plain = title_frame(&title_settings(&base));
            let titled = title_frame(&title_settings(&format!(
                "{base}\nSet WindowTitle \"{long}\""
            )));
            // Even truncated text cannot touch either button reservation.
            for y in 10..40 {
                for x in 0..width {
                    if x < 65 || x >= width.saturating_sub(65) {
                        assert_eq!(frame_pixel(&plain, x, y), frame_pixel(&titled, x, y));
                    }
                }
            }
        }
    }
    let mut multiline = title_settings("");
    multiline.presentation.style.window_title.text = "first\nsecond".to_string();
    let single = title_settings("Set WindowTitle \"first second\"");
    assert_eq!(title_frame(&multiline).pixels, title_frame(&single).pixels);
}

#[test]
fn runtime_title_patches_are_distinct_reusable_and_bounded() {
    let settings = title_settings("Set WindowTitle Initial");
    let presentation = &settings.presentation;
    let raw = Canvas::new(
        presentation.terminal_canvas_width(),
        presentation.terminal_canvas_height(),
        rgb(1, 2, 3),
    )
    .unwrap()
    .into_frame();
    let draw = |title: &str| {
        presentation
            .decorate_frame_with_title(&raw, None, &[], Some(title))
            .unwrap()
    };
    let first = draw("First");
    assert_ne!(first.pixels, draw("Second").pixels);
    assert_eq!(first.pixels, draw("First").pixels);
    assert_ne!(first.pixels, draw("").pixels);
    for index in 0..40 {
        draw(&format!("progress {index}"));
    }
    assert_eq!(presentation.style.window_title.pixels.borrow().len(), 32);
    assert_eq!(first.pixels, draw("First").pixels);
}

#[test]
fn following_app_titles_requires_a_boolean_and_defaults_off() {
    assert!(
        !title_settings("")
            .presentation
            .style
            .window_title
            .follow_terminal
    );
    assert!(
        title_settings("Set WindowTitleFromTerminal true")
            .presentation
            .style
            .window_title
            .follow_terminal
    );
    for value in ["auto", "1", "1s"] {
        let tape = Tape::parse(&format!(
            "Set Preset Default-0.1\nSet WindowTitleFromTerminal {value}"
        ))
        .unwrap();
        let error = Settings::from_tape(&tape).unwrap_err().to_string();
        assert!(
            error.contains("Set WindowTitleFromTerminal expects bool"),
            "{error}"
        );
    }
}
