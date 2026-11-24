//! The color arithmetic of `derive-palette.py`, ported so that motif fills
//! in a missing color exactly as the dotfiles' own generator does.
//!
//! These follow Python's `colorsys` module and round channels half to even,
//! as Python's `round` does, so results match it bit for bit.

use crate::Color;

/// A color as hue, lightness and saturation, each from 0 to 1.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Hls {
    pub(crate) hue: f64,
    pub(crate) light: f64,
    pub(crate) sat: f64,
}

impl Hls {
    /// Converts a color, as `colorsys.rgb_to_hls`.
    pub(crate) fn from_color(color: Color) -> Self {
        let [r, g, b] = [color.r, color.g, color.b].map(|c| f64::from(c) / 255.0);
        let max = r.max(g).max(b);
        let min = r.min(g).min(b);
        let sum = max + min;
        let range = max - min;
        let light = sum / 2.0;
        // Exact equality is what colorsys tests.
        #[allow(clippy::float_cmp)]
        if min == max {
            return Self {
                hue: 0.0,
                light,
                sat: 0.0,
            };
        }
        let sat = if light <= 0.5 {
            range / sum
        } else {
            range / (2.0 - max - min)
        };
        let rc = (max - r) / range;
        let gc = (max - g) / range;
        let bc = (max - b) / range;
        #[allow(clippy::float_cmp)]
        let hue = if r == max {
            bc - gc
        } else if g == max {
            2.0 + rc - bc
        } else {
            4.0 + gc - rc
        };
        Self {
            hue: (hue / 6.0).rem_euclid(1.0),
            light,
            sat,
        }
    }

    /// Converts back to a color, as `from_hls` in `derive-palette.py`: the
    /// hue wraps and lightness and saturation are clamped to 0..=1.
    pub(crate) fn to_color(self) -> Color {
        let hue = self.hue.rem_euclid(1.0);
        let light = self.light.clamp(0.0, 1.0);
        let sat = self.sat.clamp(0.0, 1.0);
        if sat == 0.0 {
            return rgb(light, light, light);
        }
        let m2 = if light <= 0.5 {
            light * (1.0 + sat)
        } else {
            light + sat - light * sat
        };
        let m1 = 2.0 * light - m2;
        rgb(
            channel(m1, m2, hue + 1.0 / 3.0),
            channel(m1, m2, hue),
            channel(m1, m2, hue - 1.0 / 3.0),
        )
    }
}

fn channel(m1: f64, m2: f64, hue: f64) -> f64 {
    let hue = hue.rem_euclid(1.0);
    if hue < 1.0 / 6.0 {
        m1 + (m2 - m1) * hue * 6.0
    } else if hue < 0.5 {
        m2
    } else if hue < 2.0 / 3.0 {
        m1 + (m2 - m1) * (2.0 / 3.0 - hue) * 6.0
    } else {
        m1
    }
}

/// Builds a color from channels in 0..=1.
fn rgb(r: f64, g: f64, b: f64) -> Color {
    Color::new(to_byte(r * 255.0), to_byte(g * 255.0), to_byte(b * 255.0))
}

/// Rounds half to even and clamps to a byte, as `rgb_to_hex` does.
fn to_byte(value: f64) -> u8 {
    // The clamp makes the cast exact.
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    let byte = value.round_ties_even().clamp(0.0, 255.0) as u8;
    byte
}

/// Mixes two colors channel by channel: `t = 0` gives `from`, `t = 1` gives
/// `to`.
pub(crate) fn blend(from: Color, to: Color, t: f64) -> Color {
    let mix = |a: u8, b: u8| {
        let (a, b) = (f64::from(a), f64::from(b));
        to_byte(a + (b - a) * t)
    };
    Color::new(mix(from.r, to.r), mix(from.g, to.g), mix(from.b, to.b))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Color {
        s.parse().unwrap()
    }

    // Expected values come from derive-palette.py's own functions under
    // Python 3.14.

    #[test]
    fn converts_like_colorsys() {
        let hls = Hls::from_color(hex("#cba6f7"));
        assert!((hls.hue - 0.742_798_353_909_464_9).abs() < 1e-12);
        assert!((hls.light - 0.809_803_921_568_627_5).abs() < 1e-12);
        assert!((hls.sat - 0.835_051_546_391_752_8).abs() < 1e-12);
        let grey = Hls::from_color(hex("#808080"));
        assert_eq!((grey.hue, grey.sat), (0.0, 0.0));
    }

    #[test]
    fn round_trips() {
        for s in [
            "#cba6f7", "#000000", "#ffffff", "#f38ba8", "#1e1e2e", "#7b5b55",
        ] {
            assert_eq!(Hls::from_color(hex(s)).to_color(), hex(s), "{s}");
        }
    }

    #[test]
    fn builds_the_canonical_semantic_colors() {
        let canon = |hue: f64, light, sat| {
            Hls {
                hue: hue / 360.0,
                light,
                sat,
            }
            .to_color()
            .to_string()
        };
        assert_eq!(canon(5.0, 0.72, 0.85), "#f4857b");
        assert_eq!(canon(45.0, 0.78, 0.85), "#f7df97");
        assert_eq!(canon(140.0, 0.70, 0.55), "#88dda4");
        assert_eq!(canon(42.0, 0.40, 0.70), "#ad831f");
    }

    #[test]
    fn blends_with_half_even_rounding() {
        assert_eq!(blend(hex("#1e1e2e"), hex("#cdd6f4"), 0.03), hex("#232434"));
        assert_eq!(blend(hex("#000000"), hex("#010101"), 0.5), hex("#000000"));
        assert_eq!(blend(hex("#000000"), hex("#030303"), 0.5), hex("#020202"));
    }
}
