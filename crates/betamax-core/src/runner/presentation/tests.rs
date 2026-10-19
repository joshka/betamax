use super::*;
use crate::runner::settings::Settings;
use crate::tape::Tape;

#[test]
fn caption_layout_shares_terminal_frame_edges_with_keyboard_overlay() {
    let tape = Tape::parse(
        r#"
        Set Preset Default-0.1
        Set Width 320
        Set Height 200
        Set Margin 16
        Set KeyboardOverlay Input
        Caption "Long caption"
        Type "echo done"
        "#,
    )
    .unwrap();
    let settings = Settings::from_tape(&tape).unwrap();
    let labels = vec!["\"echo done\"".to_string()];
    let avoid_width = keyboard_overlay_panel_width(&labels, settings.presentation.width)
        .saturating_add(PRESENTATION_OVERLAY_GAP);
    let layout = settings.presentation.caption_layout(avoid_width).unwrap();

    assert_eq!(layout.text_x, 16);
    assert_eq!(
        layout.text_width,
        settings
            .presentation
            .width
            .saturating_sub(32)
            .saturating_sub(avoid_width)
    );
    assert_eq!(settings.presentation.presentation_overlay_right_x(), 304);
    assert_eq!(settings.presentation.keyboard_overlay_anchor().right_x, 304);
    assert_eq!(settings.presentation.caption_overlay_left_x(), 16);
}

#[test]
fn caption_baseline_stays_put_when_keyboard_chips_come_and_go() {
    let tape = Tape::parse(
        r#"
        Set Preset Default-0.1
        Set Width 320
        Set Height 200
        Set Margin 16
        Set KeyboardOverlay Input
        Caption "Long caption"
        Type "echo done"
        "#,
    )
    .unwrap();
    let settings = Settings::from_tape(&tape).unwrap();
    let labels = vec!["\"echo done\"".to_string()];
    let avoid_width = keyboard_overlay_panel_width(&labels, settings.presentation.width)
        .saturating_add(PRESENTATION_OVERLAY_GAP);

    let with_keys = settings.presentation.caption_layout(avoid_width).unwrap();
    let without_keys = settings.presentation.caption_layout(0).unwrap();

    assert_eq!(without_keys.text_y, with_keys.text_y);
    assert_eq!(without_keys.text_height, with_keys.text_height);
}

#[test]
fn presentation_overlay_edges_track_rounded_corner_optics() {
    let tape = Tape::parse(
        r#"
        Set Preset Default-0.1
        Set Width 320
        Set Height 200
        Set Margin 16
        Set BorderRadius 8
        Set KeyboardOverlay Input
        Caption "Long caption"
        Type "echo done"
        "#,
    )
    .unwrap();
    let settings = Settings::from_tape(&tape).unwrap();
    let labels = vec!["\"echo done\"".to_string()];
    let avoid_width = keyboard_overlay_panel_width(&labels, settings.presentation.width)
        .saturating_add(PRESENTATION_OVERLAY_GAP);
    let layout = settings.presentation.caption_layout(avoid_width).unwrap();

    assert_eq!(settings.presentation.presentation_overlay_right_x(), 304);
    assert_eq!(settings.presentation.caption_overlay_left_x(), 20);
    assert_eq!(settings.presentation.keyboard_overlay_anchor().right_x, 300);
    assert_eq!(layout.text_x, 20);
}

#[test]
fn presentation_overlay_without_margin_uses_terminal_padding() {
    let tape = Tape::parse(
        r#"
        Set Preset Default-0.1
        Set Width 320
        Set Height 200
        Set Padding 20
        Set KeyboardOverlay Input
        Caption "Long caption"
        Type "echo done"
        "#,
    )
    .unwrap();
    let settings = Settings::from_tape(&tape).unwrap();

    assert_eq!(settings.presentation.caption_overlay_left_x(), 20);
    assert_eq!(settings.presentation.caption_layout(0).unwrap().text_x, 20);
    assert_eq!(settings.presentation.keyboard_overlay_anchor().right_x, 300);
}

#[test]
fn presentation_overlay_without_margin_or_padding_keeps_minimum_inset() {
    let tape = Tape::parse(
        r#"
        Set Preset Default-0.1
        Set Width 320
        Set Height 200
        Set FontSize 20
        Set Padding 0
        Caption "Long caption"
        "#,
    )
    .unwrap();
    let settings = Settings::from_tape(&tape).unwrap();

    // Half of the 18 px caption font (0.9 x FontSize 20).
    assert_eq!(settings.presentation.caption_overlay_left_x(), 9);
}

#[test]
fn presentation_overlay_with_margin_ignores_terminal_padding() {
    let tape = Tape::parse(
        r#"
        Set Preset Default-0.1
        Set Width 320
        Set Height 200
        Set Margin 16
        Set Padding 32
        Set BorderRadius 12
        Caption "Long caption"
        "#,
    )
    .unwrap();
    let settings = Settings::from_tape(&tape).unwrap();

    assert_eq!(settings.presentation.caption_overlay_left_x(), 22);
}

#[test]
fn corner_keyboard_overlay_does_not_reserve_presentation_row() {
    let tape = Tape::parse(
        r#"
        Set Preset Default-0.1
        Set Width 320
        Set Height 200
        Set Margin 16
        Set KeyboardOverlay Input
        Set KeyboardOverlayLocation BottomRight
        Type "echo done"
        "#,
    )
    .unwrap();
    let settings = Settings::from_tape(&tape).unwrap();

    assert_eq!(settings.presentation.presentation_overlay_height(), 0);
    assert_eq!(
        settings.presentation.terminal_canvas_height(),
        settings.presentation.height.saturating_sub(32)
    );
}

#[test]
fn corner_keyboard_overlay_nestles_inside_terminal_edges() {
    let tape = Tape::parse(
        r#"
        Set Preset Default-0.1
        Set Width 320
        Set Height 200
        Set Margin 16
        Set WindowBar Colorful
        Set WindowBarSize 30
        Set KeyboardOverlay Input
        Set KeyboardOverlayLocation BottomLeft
        Type "echo done"
        "#,
    )
    .unwrap();
    let settings = Settings::from_tape(&tape).unwrap();
    let anchor = settings.presentation.keyboard_overlay_anchor();

    assert!(matches!(anchor.horizontal, HorizontalAnchor::Left));
    assert!(matches!(anchor.vertical, VerticalAnchor::Bottom));
    assert_eq!(anchor.inner_left_x(), 34);
    assert_eq!(anchor.inner_bottom_y(), 166);
}

#[test]
fn caption_row_keeps_the_gradient_behind_its_text() {
    let tape = Tape::parse(
        r##"
        Set Preset Default-0.1
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
    let terminal = Canvas::new(
        settings.presentation.terminal_canvas_width(),
        settings.presentation.terminal_canvas_height(),
        super::super::canvas::rgb(255, 0, 0),
    )
    .unwrap()
    .into_frame();
    let frame = settings
        .presentation
        .decorate_frame_with_overlays(&terminal, Some("Gradient"), &[])
        .unwrap();
    let y = settings.presentation.height - settings.presentation.style.effective_margin() - 1;
    assert_eq!(pixel(&frame, 0, y), [0, 0, 0, 255]);
    assert_eq!(pixel(&frame, 160, y), [128, 128, 128, 255]);
    assert_eq!(pixel(&frame, 319, y), [255, 255, 255, 255]);
    let backdrop = settings.presentation.backdrop.clone().into_frame();
    let layout = settings.presentation.caption_layout(0).unwrap();
    assert!((layout.text_y..layout.text_y + layout.text_height)
        .any(|y| (layout.text_x..160).any(|x| pixel(&frame, x, y) != pixel(&backdrop, x, y))));
}

#[test]
fn transparent_margin_plates_the_caption_row_only_for_gif() {
    let tape = Tape::parse(
        r##"
        Set Preset Framed
        Set Width 320
        Set Height 200
        Set MarginFill transparent
        Set Shadow false
        Set KeyboardOverlay Keys
        Set KeyboardOverlayLocation CaptionRow
        Caption "Plate"
    "##,
    )
    .unwrap();
    let settings = Settings::from_tape(&tape).unwrap();
    let presentation = &settings.presentation;
    let terminal = Canvas::new(
        presentation.terminal_canvas_width(),
        presentation.terminal_canvas_height(),
        presentation.theme.background,
    )
    .unwrap()
    .into_frame();
    let labels = ["Enter".to_owned()];
    let shared = presentation
        .decorate_frame_with_overlays(&terminal, Some("Plate"), &labels)
        .unwrap();
    let delay = Duration::from_millis(40);
    let frames = [(shared.clone(), delay)];
    let gif = presentation.gif_frames(&frames).unwrap();
    let (gif_frame, gif_delay) = &gif[0];
    assert_eq!(*gif_delay, delay);

    let layout = presentation.caption_layout(0).unwrap();
    let y = layout.text_y + layout.text_height / 2;
    let bg = presentation.theme.background;
    let between = presentation.width / 2;
    // Alpha outputs keep the clear row; GIF gets an opaque theme plate behind caption and chips.
    assert_eq!(pixel(&shared, between, y)[3], 0);
    assert_eq!(pixel(gif_frame, between, y), [bg.r, bg.g, bg.b, 255]);
    assert_eq!(pixel(gif_frame, 0, y)[3], 0);
    // Opaque caption and chip pixels are drawn on top of the plate unchanged.
    for (index, (shared, gif)) in shared
        .pixels
        .chunks(4)
        .zip(gif_frame.pixels.chunks(4))
        .enumerate()
    {
        if shared[3] == 255 {
            assert_eq!(shared, gif, "pixel {index}");
        }
    }
}

#[test]
fn opaque_margin_gif_frames_are_the_shared_frames() {
    let tape = Tape::parse(
        r##"
        Set Preset Framed
        Set Width 320
        Set Height 200
        Caption "Opaque"
    "##,
    )
    .unwrap();
    let settings = Settings::from_tape(&tape).unwrap();
    let frame = Canvas::new(320, 200, settings.presentation.theme.background)
        .unwrap()
        .into_frame();
    let frames = [(frame, Duration::from_millis(40))];
    assert!(matches!(
        settings.presentation.gif_frames(&frames).unwrap(),
        std::borrow::Cow::Borrowed(_)
    ));
}

fn pixel(frame: &Frame, x: u32, y: u32) -> [u8; 4] {
    let offset = ((y * frame.width + x) * 4) as usize;
    frame.pixels[offset..offset + 4].try_into().unwrap()
}

#[test]
fn title_composes_with_other_window_effects_and_caption() {
    let base = r##"
        Set Width 360
        Set Height 240
        Set Padding 8
        Set Margin 30
        Set MarginFill "linear(to: bottom-right, #4d5b9e, #232640, #0f101a)"
        Set Shadow true
        Set ShadowBlur 14
        Set ShadowOffset 4
        Set WindowBar RingsRight
        Set WindowBarColor "#0f101a"
        Set BorderRadius 11
        Set WindowBorder 2
    "##;
    let plain = Settings::from_tape(
        &Tape::parse(&format!("Set Preset Default-0.1\n{base}\nCaption Caption")).unwrap(),
    )
    .unwrap();
    let titled = Settings::from_tape(
        &Tape::parse(&format!(
            "Set Preset Default-0.1\n{base}\nSet WindowTitle Title\nCaption Caption"
        ))
        .unwrap(),
    )
    .unwrap();
    let raw = Canvas::new(
        plain.presentation.terminal_canvas_width(),
        plain.presentation.terminal_canvas_height(),
        super::super::canvas::rgb(1, 2, 3),
    )
    .unwrap()
    .into_frame();
    let before = plain
        .presentation
        .decorate_frame_with_overlays(&raw, Some("Caption"), &[])
        .unwrap();
    let after = titled
        .presentation
        .decorate_frame_with_overlays(&raw, Some("Caption"), &[])
        .unwrap();
    assert_eq!((plain.columns, plain.rows), (titled.columns, titled.rows));
    let mut changed = 0;
    for y in 0..240 {
        for x in 0..360 {
            let offset = (y * 360 + x) * 4;
            let before_pixel = &before.pixels[offset..offset + 4];
            let after_pixel = &after.pixels[offset..offset + 4];
            if before_pixel != after_pixel {
                changed += 1;
                assert!((30..60).contains(&y));
                assert!((90..270).contains(&x));
            }
            assert_eq!(after_pixel[3], 255);
        }
    }
    assert!(changed > 0);
    assert_ne!(&after.pixels[..4], &after.pixels[after.pixels.len() - 4..]);
}

#[test]
fn decoration_preserves_every_terminal_pixel_and_grid_with_zero_padding() {
    for bar in ["", "Colorful"] {
        let source = format!(
            "Set Width 320\nSet Height 200\nSet Padding 0\nSet Margin 20\nSet WindowBar \"{bar}\""
        );
        let plain = Settings::from_tape(
            &Tape::parse(&format!("Set Preset Default-0.1\n{source}")).unwrap(),
        )
        .unwrap();
        let decorated = Settings::from_tape(
            &Tape::parse(&format!(
                "Set Preset Default-0.1\n{source}\nSet BorderRadius 11\nSet WindowBorder 2\nSet WindowBorderOpacity 1"
            ))
            .unwrap(),
        )
        .unwrap();
        assert_eq!(
            (plain.columns, plain.rows),
            (decorated.columns, decorated.rows)
        );
        let raw = Canvas::new(
            plain.presentation.terminal_canvas_width(),
            plain.presentation.terminal_canvas_height(),
            super::super::canvas::rgb(17, 203, 79),
        )
        .unwrap()
        .into_frame();
        let output = decorated
            .presentation
            .decorate_frame_with_overlays(&raw, None, &[])
            .unwrap();
        assert_eq!((output.width, output.height), (320, 200));
        let top = 20 + decorated.presentation.style.window_bar_height();
        for y in 0..raw.height {
            let start = (((top + y) * output.width + 20) * 4) as usize;
            let source = (y * raw.width * 4) as usize;
            assert_eq!(
                &output.pixels[start..start + raw.width as usize * 4],
                &raw.pixels[source..source + raw.width as usize * 4],
                "terminal row {y} changed with bar {bar}"
            );
        }
    }
}

#[test]
fn decoration_rejects_insufficient_margin_instead_of_cropping_cells() {
    let tape =
        Tape::parse("Set Preset Default-0.1\nSet Padding 0\nSet Margin 0\nSet BorderRadius 11")
            .unwrap();
    let error = Settings::from_tape(&tape).unwrap_err().to_string();
    assert!(error.contains("increase Margin"), "{error}");
}
