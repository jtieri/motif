//! The status colors: error, warning and success.

use std::fmt;
use std::str::FromStr;

use super::hls::Hls;
use crate::{Color, Mode};

/// How the error, warning and success colors are chosen.
///
/// These follow `theme-generate`'s `--semantic` option and the
/// `_semantic_base` function of `derive-palette.py`, which this ports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum Semantic {
    /// The theme's own red, yellow and green, even where a theme's "red" is
    /// not red. This keeps a theme as its author made it.
    #[default]
    Literal,
    /// A canonical red, yellow and green hue at the lightness and saturation
    /// of the theme's own colors.
    Tinted,
    /// Canonical colors for the mode, whatever the theme.
    Fixed,
}

impl Semantic {
    /// Returns `"literal"`, `"tinted"` or `"fixed"`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Literal => "literal",
            Self::Tinted => "tinted",
            Self::Fixed => "fixed",
        }
    }
}

impl fmt::Display for Semantic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Semantic {
    type Err = ParseSemanticError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "literal" => Ok(Self::Literal),
            "tinted" => Ok(Self::Tinted),
            "fixed" => Ok(Self::Fixed),
            _ => Err(ParseSemanticError(())),
        }
    }
}

/// The error returned when a string is not `literal`, `tinted` or `fixed`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("expected literal, tinted or fixed")]
pub struct ParseSemanticError(());

/// A canonical status color: its hue in degrees, and its HLS lightness and
/// saturation in a dark and in a light theme. These are `_CANON` in
/// `derive-palette.py`.
struct Canon {
    dark: (f64, f64, f64),
    light: (f64, f64, f64),
}

const ERROR: Canon = Canon {
    dark: (5.0, 0.72, 0.85),
    light: (5.0, 0.50, 0.70),
};
const WARNING: Canon = Canon {
    dark: (45.0, 0.78, 0.85),
    light: (42.0, 0.40, 0.70),
};
const SUCCESS: Canon = Canon {
    dark: (140.0, 0.70, 0.55),
    light: (140.0, 0.34, 0.55),
};

/// The three status colors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Status {
    pub(crate) error: Color,
    pub(crate) warning: Color,
    pub(crate) success: Color,
}

impl Status {
    /// Chooses the status colors for a theme in `mode` whose red, yellow and
    /// green are given.
    pub(crate) fn new(
        semantic: Semantic,
        mode: Mode,
        red: Color,
        yellow: Color,
        green: Color,
    ) -> Self {
        let base = |canon: &Canon, own: Color| {
            let (hue, light, sat) = match mode {
                Mode::Dark => canon.dark,
                Mode::Light => canon.light,
            };
            let hue = hue / 360.0;
            match semantic {
                Semantic::Literal => own,
                Semantic::Tinted => {
                    let own = Hls::from_color(own);
                    Hls {
                        hue,
                        light: own.light,
                        sat: own.sat,
                    }
                    .to_color()
                }
                Semantic::Fixed => Hls { hue, light, sat }.to_color(),
            }
        };
        Self {
            error: base(&ERROR, red),
            warning: base(&WARNING, yellow),
            success: base(&SUCCESS, green),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Color {
        s.parse().unwrap()
    }

    // Expected values come from derive-palette.py's _semantic_base.

    #[test]
    fn keeps_literal_colors() {
        let status = Status::new(
            Semantic::Literal,
            Mode::Dark,
            hex("#4d86b0"),
            hex("#6fa4c9"),
            hex("#5e95bc"),
        );
        assert_eq!(status.error, hex("#4d86b0"));
        assert_eq!(status.warning, hex("#6fa4c9"));
        assert_eq!(status.success, hex("#5e95bc"));
    }

    #[test]
    fn tints_canonical_hues() {
        let status = Status::new(
            Semantic::Tinted,
            Mode::Dark,
            hex("#89b4fa"),
            hex("#89b4fa"),
            hex("#89b4fa"),
        );
        assert_eq!(status.error, hex("#fa9289"));
    }

    #[test]
    fn uses_canonical_colors_when_fixed() {
        let any = Color::BLACK;
        let dark = Status::new(Semantic::Fixed, Mode::Dark, any, any, any);
        assert_eq!(
            [dark.error, dark.warning, dark.success].map(|c| c.to_string()),
            ["#f4857b", "#f7df97", "#88dda4"]
        );
        let light = Status::new(Semantic::Fixed, Mode::Light, any, any, any);
        assert_eq!(
            [light.error, light.warning, light.success].map(|c| c.to_string()),
            ["#d93526", "#ad831f", "#278647"]
        );
    }

    #[test]
    fn parses_and_displays() {
        for semantic in [Semantic::Literal, Semantic::Tinted, Semantic::Fixed] {
            assert_eq!(semantic.as_str().parse(), Ok(semantic));
        }
        assert!("Fixed".parse::<Semantic>().is_err());
    }
}
