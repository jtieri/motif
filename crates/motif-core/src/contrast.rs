//! Contrast between two colors, as WCAG 2 defines it.

use crate::Color;

/// The contrast WCAG 2 asks of body text, 4.5:1.
pub const TEXT: f64 = 4.5;

/// The contrast WCAG 2 asks of large text and of the parts of a control
/// that show its state, 3:1.
pub const NON_TEXT: f64 = 3.0;

/// Returns the relative luminance of a color, from 0 for black to 1 for
/// white.
#[must_use]
pub fn relative_luminance(color: Color) -> f64 {
    let linear = |channel: u8| {
        let c = f64::from(channel) / 255.0;
        if c <= 0.040_45 {
            c / 12.92
        } else {
            ((c + 0.055) / 1.055).powf(2.4)
        }
    };
    0.2126 * linear(color.r) + 0.7152 * linear(color.g) + 0.0722 * linear(color.b)
}

/// Returns the contrast ratio of two colors, from 1 for equal luminance to 21
/// for black on white. The order of the colors does not matter.
///
/// # Examples
///
/// ```
/// use motif_core::Color;
/// use motif_core::contrast::contrast_ratio;
///
/// assert_eq!(contrast_ratio(Color::BLACK, Color::WHITE), 21.0);
/// assert_eq!(contrast_ratio(Color::WHITE, Color::WHITE), 1.0);
/// ```
#[must_use]
pub fn contrast_ratio(a: Color, b: Color) -> f64 {
    let (a, b) = (relative_luminance(a), relative_luminance(b));
    let (light, dark) = if a > b { (a, b) } else { (b, a) };
    (light + 0.05) / (dark + 0.05)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matches_known_ratios() {
        let ratio = |a: &str, b: &str| contrast_ratio(a.parse().unwrap(), b.parse().unwrap());
        // Catppuccin Mocha's text on its base.
        assert!((ratio("#cdd6f4", "#1e1e2e") - 11.34).abs() < 0.01);
        assert!((ratio("#777777", "#ffffff") - 4.48).abs() < 0.01);
        assert_eq!(ratio("#123456", "#abcdef"), ratio("#abcdef", "#123456"));
    }
}
