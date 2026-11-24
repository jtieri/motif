//! An sRGB color with 8 bits per channel.

use std::fmt;
use std::str::FromStr;

/// An opaque sRGB color with 8 bits per channel.
///
/// Parses from and displays as `#rrggbb`. Parsing accepts either case;
/// display is always lowercase.
///
/// # Examples
///
/// ```
/// use motif_core::Color;
///
/// let teal: Color = "#7DAEA3".parse()?;
/// assert_eq!(teal, Color::new(0x7d, 0xae, 0xa3));
/// assert_eq!(teal.to_string(), "#7daea3");
/// # Ok::<(), motif_core::ParseColorError>(())
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Color {
    /// The red channel.
    pub r: u8,
    /// The green channel.
    pub g: u8,
    /// The blue channel.
    pub b: u8,
}

impl Color {
    /// Black, `#000000`.
    pub const BLACK: Self = Self::new(0, 0, 0);
    /// White, `#ffffff`.
    pub const WHITE: Self = Self::new(0xff, 0xff, 0xff);

    /// Creates a color from its red, green and blue channels.
    #[must_use]
    pub const fn new(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }

    /// Parses exactly six hex digits, without a leading `#`.
    pub(crate) fn from_hex6(digits: &str) -> Option<Self> {
        if digits.len() != 6 || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
            return None;
        }
        let channel = |i: usize| u8::from_str_radix(&digits[i..i + 2], 16).ok();
        Some(Self::new(channel(0)?, channel(2)?, channel(4)?))
    }
}

impl fmt::Display for Color {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "#{:02x}{:02x}{:02x}", self.r, self.g, self.b)
    }
}

impl FromStr for Color {
    type Err = ParseColorError;

    /// Parses `#rrggbb`. Hex digits may be in either case.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        s.strip_prefix('#')
            .and_then(Self::from_hex6)
            .ok_or(ParseColorError(()))
    }
}

/// The error returned when a string is not a `#rrggbb` color.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("expected a color in the form #rrggbb")]
pub struct ParseColorError(());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_either_case() {
        assert_eq!("#aBcDeF".parse(), Ok(Color::new(0xab, 0xcd, 0xef)));
        assert_eq!("#000000".parse(), Ok(Color::BLACK));
    }

    #[test]
    fn displays_lowercase() {
        assert_eq!(Color::new(0xab, 0x0c, 0xff).to_string(), "#ab0cff");
    }

    #[test]
    fn rejects_other_forms() {
        for s in [
            "", "#", "abcdef", "#abcde", "#abcdeff", "#abcdeg", "#ab cde", "#+bcdef", "#ÿbcde",
        ] {
            assert!(s.parse::<Color>().is_err(), "{s:?}");
        }
    }
}
