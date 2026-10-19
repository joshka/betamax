//! OKLCH conversion for theme-derived surfaces.
//!
//! OKLCH separates perceived lightness, chroma, and hue, so a surface can be darkened or made more
//! colorful while keeping the theme background's own hue.

use libghostty_vt::style::RgbColor;

use super::canvas::rgb;

/// A color in OKLCH: lightness `0..=1`, chroma `0..`, and hue in degrees.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(super) struct Oklch {
    pub(super) lightness: f64,
    pub(super) chroma: f64,
    pub(super) hue: f64,
}

impl Oklch {
    /// Convert an sRGB color to OKLCH.
    pub(super) fn from_rgb(color: RgbColor) -> Self {
        let r = srgb_to_linear(color.r);
        let g = srgb_to_linear(color.g);
        let b = srgb_to_linear(color.b);
        let l = (0.412_221_470_8 * r + 0.536_332_536_3 * g + 0.051_445_992_9 * b).cbrt();
        let m = (0.211_903_498_2 * r + 0.680_699_545_1 * g + 0.107_396_956_6 * b).cbrt();
        let s = (0.088_302_461_9 * r + 0.281_718_837_6 * g + 0.629_978_700_5 * b).cbrt();
        let lightness = 0.210_454_255_3 * l + 0.793_617_785_0 * m - 0.004_072_046_8 * s;
        let a = 1.977_998_495_1 * l - 2.428_592_205_0 * m + 0.450_593_709_9 * s;
        let b = 0.025_904_037_1 * l + 0.782_771_766_2 * m - 0.808_675_766_0 * s;
        Self {
            lightness,
            chroma: a.hypot(b),
            hue: b.atan2(a).to_degrees(),
        }
    }

    /// Convert to sRGB, clamping out-of-gamut channels.
    pub(super) fn to_rgb(self) -> RgbColor {
        let (sin, cos) = self.hue.to_radians().sin_cos();
        let a = self.chroma * cos;
        let b = self.chroma * sin;
        let l = (self.lightness + 0.396_337_777_4 * a + 0.215_803_757_3 * b).powi(3);
        let m = (self.lightness - 0.105_561_345_8 * a - 0.063_854_172_8 * b).powi(3);
        let s = (self.lightness - 0.089_484_177_5 * a - 1.291_485_548_0 * b).powi(3);
        rgb(
            linear_to_srgb(4.076_741_662_1 * l - 3.307_711_591_3 * m + 0.230_969_929_2 * s),
            linear_to_srgb(-1.268_438_004_6 * l + 2.609_757_401_1 * m - 0.341_319_396_5 * s),
            linear_to_srgb(-0.004_196_086_3 * l - 0.703_418_614_7 * m + 1.707_614_701_0 * s),
        )
    }
}

fn srgb_to_linear(channel: u8) -> f64 {
    let value = f64::from(channel) / 255.0;
    if value <= 0.04045 {
        value / 12.92
    } else {
        ((value + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(value: f64) -> u8 {
    let value = value.clamp(0.0, 1.0);
    let encoded = if value <= 0.003_130_8 {
        12.92 * value
    } else {
        1.055 * value.powf(1.0 / 2.4) - 0.055
    };
    (encoded * 255.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_srgb_colors() {
        for color in [
            rgb(0, 0, 0),
            rgb(255, 255, 255),
            rgb(0x0f, 0x14, 0x1f),
            rgb(0xfb, 0xf1, 0xc7),
            rgb(0xc2, 0x62, 0x65),
        ] {
            assert_eq!(Oklch::from_rgb(color).to_rgb(), color);
        }
    }

    #[test]
    fn neutral_colors_have_no_chroma() {
        let gray = Oklch::from_rgb(rgb(0x28, 0x28, 0x28));
        assert!(gray.chroma < 1e-4, "{gray:?}");
        assert!((Oklch::from_rgb(rgb(255, 255, 255)).lightness - 1.0).abs() < 1e-4);
    }
}
