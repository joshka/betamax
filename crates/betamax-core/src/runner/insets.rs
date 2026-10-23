//! Per-side pixel insets for `Set Margin`, using CSS shorthand.

use miette::miette;

use crate::Result;

/// Pixel distances from each output edge to the window.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) struct Insets {
    pub(super) top: u32,
    pub(super) right: u32,
    pub(super) bottom: u32,
    pub(super) left: u32,
}

impl Insets {
    /// The same inset on every side, as `Set Margin 40` sets.
    pub(super) const fn uniform(value: u32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    /// Parse CSS margin shorthand: 1–4 space-separated non-negative integers.
    ///
    /// One value sets every side; two set top/bottom then left/right; three set top, left/right,
    /// then bottom; four set top, right, bottom, left.
    pub(super) fn parse(value: &str) -> Result<Self> {
        let invalid = || {
            miette!(
                "Set Margin expects 1 to 4 space-separated pixel values (top right bottom left), got `{value}`"
            )
        };
        let values = value
            .split_whitespace()
            .map(|part| part.parse::<u32>().map_err(|_| invalid()))
            .collect::<std::result::Result<Vec<_>, _>>()?;
        let [top, right, bottom, left] = match values[..] {
            [all] => [all; 4],
            [vertical, horizontal] => [vertical, horizontal, vertical, horizontal],
            [top, horizontal, bottom] => [top, horizontal, bottom, horizontal],
            [top, right, bottom, left] => [top, right, bottom, left],
            _ => return Err(invalid().into()),
        };
        Ok(Self {
            top,
            right,
            bottom,
            left,
        })
    }

    /// Combined left and right insets.
    pub(super) fn horizontal(self) -> u32 {
        self.left.saturating_add(self.right)
    }

    /// Combined top and bottom insets.
    pub(super) fn vertical(self) -> u32 {
        self.top.saturating_add(self.bottom)
    }

    /// The narrowest side, which bounds how far decoration can grow on every side.
    pub(super) fn min_side(self) -> u32 {
        self.top.min(self.right).min(self.bottom).min(self.left)
    }

    /// Whether any side has a margin.
    pub(super) fn any(self) -> bool {
        self != Self::default()
    }
}

#[cfg(test)]
mod tests {
    use super::Insets;

    fn sides(insets: Insets) -> [u32; 4] {
        [insets.top, insets.right, insets.bottom, insets.left]
    }

    #[test]
    fn shorthand_follows_css_side_order() {
        for (source, expected) in [
            ("40", [40, 40, 40, 40]),
            ("10 20", [10, 20, 10, 20]),
            ("10 20 30", [10, 20, 30, 20]),
            ("150 288 326 288", [150, 288, 326, 288]),
            ("  1   2 3  4 ", [1, 2, 3, 4]),
        ] {
            assert_eq!(
                sides(Insets::parse(source).unwrap()),
                expected,
                "{source:?}"
            );
        }
    }

    #[test]
    fn malformed_shorthand_names_the_expected_form() {
        for source in ["", "   ", "1 2 3 4 5", "-1", "10px", "1.5", "a b"] {
            let error = Insets::parse(source).unwrap_err().to_string();
            assert!(
                error.contains("Set Margin expects 1 to 4 space-separated pixel values"),
                "{source:?}: {error}"
            );
        }
    }
}
