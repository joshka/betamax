use super::*;
use crate::runner::settings::Settings;

fn settings(source: &str) -> Settings {
    Settings::from_tape(&Tape::parse(source).unwrap()).unwrap()
}

#[test]
fn implicit_and_explicit_framed_share_the_complete_baseline() {
    let implicit = settings("");
    let explicit = settings("Set Preset Framed");
    let default = settings("Set Preset Default");
    assert_eq!(implicit.presentation.style, default.presentation.style);
    assert_eq!(
        (implicit.columns, implicit.rows),
        (default.columns, default.rows)
    );
    assert_eq!(implicit.presentation.style, explicit.presentation.style);
    assert_eq!((implicit.columns, implicit.rows), (87, 28));
    assert_eq!(implicit.presentation.theme.name, "Aardvark Ink");
    assert_eq!(implicit.presentation.text.font_size, 22.0);
    assert_eq!(implicit.presentation.text.padding, 0);
    assert_eq!(
        (implicit.presentation.width, implicit.presentation.height),
        (1200, 720)
    );
    assert!(implicit.presentation.style.window_title.follow_terminal);
}

#[test]
fn explicit_values_override_presets_in_either_order_and_last_value_wins() {
    for source in [
        "Set Margin 44\nSet Padding 0\nSet Shadow false\nSet WindowBar \"\"\nSet WindowBorder 0\nSet Preset Framed",
        "Set Preset Framed\nSet Margin 44\nSet Padding 0\nSet Shadow false\nSet WindowBar \"\"\nSet WindowBorder 0",
    ] {
        let settings = settings(source);
        let style = &settings.presentation.style;
        assert_eq!(style.margin, Insets::uniform(44));
        assert_eq!(settings.presentation.text.padding, 0);
        assert!(!style.shadow.enabled);
        assert!(style.window_bar.is_empty());
        assert_eq!(style.border.width, 0);
    }
    let repeated = settings("Set Margin 44\nSet Preset Bare\nSet Margin 12\nSet Preset Framed");
    assert_eq!(repeated.presentation.style.margin, Insets::uniform(12));
}

#[test]
fn presets_have_distinct_geometry_and_versioned_default_restores_old_defaults() {
    let bare = settings("Set Preset Bare");
    assert_eq!(
        (
            bare.presentation.style.margin,
            bare.presentation.text.padding
        ),
        (Insets::default(), 0)
    );
    assert_eq!((bare.columns, bare.rows), (92, 32));
    let inset = settings("Set Preset Inset");
    assert_eq!(inset.presentation.style.margin, Insets::uniform(16));
    assert!(inset.presentation.style.window_bar.is_empty());
    assert!(!inset.presentation.style.shadow.enabled);
    let legacy = settings("Set Preset Default-0.1");
    assert_eq!(legacy.presentation.theme.name, "Aardvark Blue");
    assert_eq!(legacy.presentation.height, 600);
    assert_eq!(legacy.presentation.text.font_size, 22.0);
    assert_eq!(legacy.presentation.text.padding, 60);
    assert!(!legacy.presentation.style.window_title.follow_terminal);
}

#[test]
fn final_theme_drives_surfaces_without_erasing_explicit_colors() {
    for source in [
        "Set MarginFill \"#123456\"\nSet WindowBarColor \"#abcdef\"\nSet Theme \"GitHub Light Default\"\nSet Preset Framed",
        "Set Preset Framed\nSet Theme \"GitHub Light Default\"\nSet MarginFill \"#123456\"\nSet WindowBarColor \"#abcdef\"",
    ] {
        let result = settings(source);
        assert_eq!(result.presentation.style.margin_fill.as_deref(), Some("#123456"));
        assert_eq!(result.presentation.style.window_bar_color_override.as_deref(), Some("#abcdef"));
    }
    let light = settings("Set Theme \"GitHub Light Default\"");
    let dark = settings("");
    assert_ne!(
        light.presentation.style.margin_fill,
        dark.presentation.style.margin_fill
    );
}

#[test]
fn invalid_presets_fail_even_when_overridden() {
    for source in [
        "Set Preset Unknown",
        "Set Preset 1",
        "Set Preset false",
        "Set Preset Unknown\nSet Preset Bare",
    ] {
        assert!(
            Settings::from_tape(&Tape::parse(source).unwrap()).is_err(),
            "{source}"
        );
    }
}

#[test]
fn tiny_framed_dimensions_fail_before_capture_instead_of_panicking() {
    let tape = Tape::parse("Set Width 80\nSet Height 60").unwrap();
    let error = Settings::from_tape(&tape).unwrap_err().to_string();
    assert!(error.contains("leave no terminal canvas"), "{error}");
    let bare = settings("Set Preset Bare\nSet Width 80\nSet Height 60");
    assert!(bare.columns > 0 && bare.rows > 0);
}

#[test]
fn last_default_preset_restores_the_default_baseline_without_erasing_overrides() {
    let selected = settings("Set Preset Bare\nSet Margin 44\nSet Preset Default");
    assert_eq!(selected.presentation.style.margin, Insets::uniform(44));
    assert_eq!(selected.presentation.style.window_bar, "Colorful");
    assert!(selected.presentation.style.shadow.enabled);
}

#[test]
fn framed_gradient_follows_the_background_hue_and_stays_darker_than_the_window() {
    for (theme, gradient) in [
        ("Aardvark Ink", "linear(#060d1e, #040714)"),
        ("Nord", "linear(#232c3f, #151c2a)"),
        ("Gruvbox Dark", "linear(#212121, #151515)"),
        ("Gruvbox Light", "linear(#f0e7c2, #d9d0ab)"),
        ("GitHub Light Default", "linear(#f5f5f5, #dedede)"),
    ] {
        let settings = settings(&format!("Set Theme \"{theme}\""));
        assert_eq!(
            settings.presentation.style.margin_fill.as_deref(),
            Some(gradient),
            "{theme}"
        );
    }
    let inset = settings("Set Preset Inset");
    assert_eq!(
        inset.presentation.style.margin_fill.as_deref(),
        Some("#0b0f17")
    );
    assert_eq!(inset.presentation.style.window_bar_color, "#0b0f17");
}
