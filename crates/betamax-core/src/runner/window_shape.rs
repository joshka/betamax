//! Continuous window geometry shared by masks, inset borders, and shadows.
//!
//! Three cubic segments ease each corner into its straight edges. The normalized control
//! points match the continuous rounded-rectangle profile inspected in SwiftUI; rendering is
//! portable and has no platform-framework dependency.

use super::canvas::PixelRect;
use crate::Result;

// Corner reach relative to its nominal radius. Clamp before generating either coverage or shadow.
const REACH: f32 = 1.528_665;
const CORNER: [[(f32, f32); 3]; 3] = [
    [(1.088_49, 0.0), (0.868_407, 0.0), (0.631_494, 0.074_911)],
    [
        (0.372_824, 0.169_06),
        (0.169_06, 0.372_824),
        (0.074_911, 0.631_494),
    ],
    [(0.0, 0.868_407), (0.0, 1.088_49), (0.0, REACH)],
];

/// One continuous rectangle, with the same radius clamp for every consumer.
#[derive(Debug, Clone)]
pub(super) struct WindowShape {
    rect: PixelRect,
    radius: f32,
}

impl WindowShape {
    pub(super) fn new(rect: PixelRect, radius: u32) -> Self {
        Self::with_radius(rect, radius as f32)
    }

    fn with_radius(rect: PixelRect, radius: f32) -> Self {
        let limit = (rect.width.min(rect.height) / 2) as f32 / REACH;
        Self {
            rect,
            radius: radius.min(limit),
        }
    }

    /// Cache just one corner; reflect its coverage for exact four-way symmetry.
    pub(super) fn mask(&self) -> Result<WindowMask> {
        let size = (self.radius * REACH).ceil() as u32;
        if size == 0 {
            return Ok(WindowMask {
                rect: self.rect,
                size,
                corner: Vec::new(),
            });
        }
        let mut builder = tiny_skia::PathBuilder::new();
        builder.move_to(self.radius * REACH, 0.0);
        for points in CORNER {
            builder.cubic_to(
                points[0].0 * self.radius,
                points[0].1 * self.radius,
                points[1].0 * self.radius,
                points[1].1 * self.radius,
                points[2].0 * self.radius,
                points[2].1 * self.radius,
            );
        }
        builder.line_to(0.0, size as f32);
        builder.line_to(size as f32, size as f32);
        builder.line_to(size as f32, 0.0);
        builder.close();
        let path = builder.finish().expect("nonempty continuous corner");
        let corner = super::canvas::path_coverage(&path, size, size)?;
        Ok(WindowMask {
            rect: self.rect,
            size,
            corner,
        })
    }

    /// Approximate the continuous curve with short segments for analytic shadow falloff.
    pub(super) fn shadow_outline(&self) -> ShadowOutline {
        let mut points = vec![(self.radius * REACH, 0.0)];
        let mut start = points[0];
        for control in CORNER {
            let control = control.map(|(x, y)| (x * self.radius, y * self.radius));
            for step in 1..=12 {
                let t = step as f32 / 12.0;
                let u = 1.0 - t;
                let point = |a: f32, b: f32, c: f32, d: f32| {
                    u * u * u * a + 3.0 * u * u * t * b + 3.0 * u * t * t * c + t * t * t * d
                };
                points.push((
                    point(start.0, control[0].0, control[1].0, control[2].0),
                    point(start.1, control[0].1, control[1].1, control[2].1),
                ));
            }
            start = control[2];
        }
        ShadowOutline {
            rect: self.rect,
            reach: self.radius * REACH,
            points,
        }
    }
}

/// Cached coverage in output coordinates, independent of backdrop and window colors.
#[derive(Debug, Clone)]
pub(super) struct WindowMask {
    rect: PixelRect,
    size: u32,
    corner: Vec<u8>,
}

impl WindowMask {
    pub(super) fn coverage(&self, x: u32, y: u32) -> f64 {
        let rect = self.rect;
        if x < rect.x || y < rect.y || x - rect.x >= rect.width || y - rect.y >= rect.height {
            return 0.0;
        }
        let dx = (x - rect.x).min(rect.width - 1 - (x - rect.x));
        let dy = (y - rect.y).min(rect.height - 1 - (y - rect.y));
        if dx >= self.size || dy >= self.size {
            return 1.0;
        }
        f64::from(self.corner[(dy * self.size + dx) as usize]) / 255.0
    }
}

/// A startup-only distance evaluator for the same continuous profile as the window mask.
#[derive(Debug)]
pub(super) struct ShadowOutline {
    rect: PixelRect,
    reach: f32,
    points: Vec<(f32, f32)>,
}

impl ShadowOutline {
    pub(super) fn distance(&self, x: f32, y: f32) -> f64 {
        let x = (x - self.rect.x as f32).min((self.rect.x + self.rect.width) as f32 - x);
        let y = (y - self.rect.y as f32).min((self.rect.y + self.rect.height) as f32 - y);
        if self.reach == 0.0 {
            return f64::from((-x).max(0.0).hypot((-y).max(0.0)) + (-x).max(-y).min(0.0));
        }
        if x >= self.reach || y >= self.reach {
            return f64::from((-x).max(-y));
        }
        let mut distance = f32::INFINITY;
        let mut inside = true;
        for line in self.points.windows(2) {
            let (ax, ay) = line[0];
            let (bx, by) = line[1];
            let dx = bx - ax;
            let dy = by - ay;
            let length_sq = dx * dx + dy * dy;
            if length_sq == 0.0 {
                continue;
            }
            let t = (((x - ax) * dx + (y - ay) * dy) / length_sq).clamp(0.0, 1.0);
            distance = distance.min((x - ax - t * dx).hypot(y - ay - t * dy));
            inside &= dx * (y - ay) - dy * (x - ax) <= 0.0;
        }
        f64::from(if inside { -distance } else { distance })
    }
}

/// Prepared outer/inner coverage shared by the border and unbordered window mask.
#[derive(Debug, Clone)]
pub(super) struct WindowOutline {
    pub(super) rect: PixelRect,
    pub(super) outer: WindowMask,
    pub(super) inner: WindowMask,
}

impl WindowOutline {
    pub(super) fn new(rect: PixelRect, radius: u32, width: u32) -> Result<Self> {
        let inset = width
            .min(rect.width.div_ceil(2))
            .min(rect.height.div_ceil(2));
        let inner = PixelRect {
            x: rect.x.saturating_add(inset),
            y: rect.y.saturating_add(inset),
            width: rect.width.saturating_sub(2 * inset),
            height: rect.height.saturating_sub(2 * inset),
        };
        let shape = WindowShape::new(rect, radius);
        Ok(Self {
            rect,
            outer: shape.mask()?,
            inner: WindowShape::with_radius(inner, (shape.radius - inset as f32).max(0.0))
                .mask()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn continuous_coverage_tracks_native_reference() {
        let mask = WindowShape::new(
            PixelRect {
                x: 0,
                y: 0,
                width: 100,
                height: 100,
            },
            11,
        )
        .mask()
        .unwrap();
        // Independent SwiftUI .continuous path, rasterized by Core Graphics at 64× density.
        // Allow rasterizer differences, not a different corner profile.
        for (x, y, expected) in [
            (0, 7, 0.3359),
            (0, 12, 0.9636),
            (1, 6, 0.9458),
            (2, 4, 0.8419),
            (4, 2, 0.8419),
            (7, 0, 0.3359),
            (12, 0, 0.9636),
            (16, 0, 1.0),
        ] {
            assert!(
                (mask.coverage(x, y) - expected).abs() < 0.025,
                "{x},{y}: {}",
                mask.coverage(x, y)
            );
        }
    }

    #[test]
    fn corners_are_symmetric_and_inset_coverage_stays_inside() {
        for (width, height) in [(0, 0), (1, 1), (2, 3), (9, 8), (40, 31), (96, 48)] {
            for radius in [0, 1, 2, 11, 100] {
                for inset in [0, 1, 2, u32::MAX] {
                    let rect = PixelRect {
                        x: 3,
                        y: 5,
                        width,
                        height,
                    };
                    let outline = WindowOutline::new(rect, radius, inset).unwrap();
                    for y in 0..height {
                        for x in 0..width {
                            let a = outline.outer.coverage(3 + x, 5 + y);
                            assert_eq!(a, outline.outer.coverage(3 + width - 1 - x, 5 + y));
                            assert_eq!(a, outline.outer.coverage(3 + x, 5 + height - 1 - y));
                            assert!(outline.inner.coverage(3 + x, 5 + y) <= a + 1.0 / 255.0);
                        }
                    }
                    assert_eq!(outline.outer.coverage(2, 5), 0.0);
                    assert_eq!(outline.outer.coverage(3 + width, 5), 0.0);
                }
            }
        }
    }

    #[test]
    fn shadow_distance_agrees_with_mask_and_is_symmetric() {
        let rect = PixelRect {
            x: 10,
            y: 10,
            width: 100,
            height: 70,
        };
        let shape = WindowShape::new(rect, 11);
        let mask = shape.mask().unwrap();
        let shadow = shape.shadow_outline();
        for y in 0..70 {
            for x in 0..100 {
                let distance = shadow.distance(10.5 + x as f32, 10.5 + y as f32);
                let mirrored = shadow.distance(109.5 - x as f32, 79.5 - y as f32);
                assert!((distance - mirrored).abs() < 0.0001);
                if distance < -0.8 {
                    assert_eq!(mask.coverage(x + 10, y + 10), 1.0);
                }
                if distance > 0.8 {
                    assert_eq!(mask.coverage(x + 10, y + 10), 0.0);
                }
            }
        }
        assert!(shadow.distance(9.0, 9.0) > 0.0);
        let square = WindowShape::new(rect, 0).shadow_outline();
        assert!((square.distance(9.0, 9.0) - 2.0_f64.sqrt()).abs() < 0.0001);
    }
}
