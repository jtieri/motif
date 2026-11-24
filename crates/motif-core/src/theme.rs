//! The neutral theme model that every source fills.
//!
//! A source translates its own format into a [`Theme`] and nothing else reads
//! the format. Fields a format has no value for are `None`; sources never make
//! up a value to fill one.

use std::fmt;
use std::path::PathBuf;

use crate::Color;

/// A color theme read from a source.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    /// The theme's display name.
    pub name: String,
    /// Whether the theme is dark or light.
    pub mode: Mode,
    /// The 16 terminal colors, `color0` to `color15`.
    pub ansi: [Color; 16],
    /// The background tiers.
    pub background: Background,
    /// The foreground tiers.
    pub foreground: Foreground,
    /// The color for focused and highlighted elements.
    pub accent: Option<Color>,
    /// The colors of selected text.
    pub selection: Selection,
    /// The color for de-emphasized text such as comments.
    pub muted: Option<Color>,
    /// The cursor color.
    pub cursor: Option<Color>,
    /// The hues of the theme's palette, under the source's own names and in
    /// its order.
    pub hues: Vec<NamedColor>,
    /// Colors the source names that are not part of its palette, such as a
    /// window border or an icon tint for one application, in the source's
    /// order.
    pub extras: Vec<NamedColor>,
    /// Where the theme came from.
    pub provenance: Provenance,
}

/// Whether a theme has a dark or a light background.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Mode {
    /// Light text on a dark background.
    Dark,
    /// Dark text on a light background.
    Light,
}

impl Mode {
    /// Returns `"dark"` or `"light"`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dark => "dark",
            Self::Light => "light",
        }
    }
}

impl fmt::Display for Mode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A theme's background colors.
///
/// The names follow Omarchy's: `dark` and `darker` sit behind the main
/// background (sidebars, inactive panes) and `lighter` sits above it (raised
/// panels, the current line). In a light theme they are still named this way
/// even where the colors do not get darker or lighter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Background {
    /// The main background.
    pub base: Color,
    /// A recessed surface.
    pub dark: Option<Color>,
    /// A surface recessed further than `dark`.
    pub darker: Option<Color>,
    /// A raised surface.
    pub lighter: Option<Color>,
}

/// A theme's foreground colors.
///
/// The names follow Omarchy's: `dark` is a dim text tier, `light` a secondary
/// one, and `bright` the most emphasized.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Foreground {
    /// The main text color.
    pub base: Color,
    /// Dim text.
    pub dark: Option<Color>,
    /// Secondary text.
    pub light: Option<Color>,
    /// Emphasized text.
    pub bright: Option<Color>,
}

/// The colors of selected text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Selection {
    /// The text color of a selection.
    pub foreground: Option<Color>,
    /// The background of a selection.
    pub background: Option<Color>,
}

/// A color under the name a source gives it, such as `orange` or `base0E`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NamedColor {
    /// The source's name for the color.
    pub name: String,
    /// The color.
    pub color: Color,
}

/// Where a theme came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Provenance {
    /// The format the theme was read from.
    pub kind: SourceKind,
    /// The repository or page the theme was fetched from.
    pub url: Option<String>,
    /// The commit the theme was read at.
    pub commit: Option<String>,
    /// The local file or folder the theme was read from.
    pub path: Option<PathBuf>,
}

impl Provenance {
    /// Returns the provenance of a theme read from a local path.
    #[must_use]
    pub fn local(kind: SourceKind, path: impl Into<PathBuf>) -> Self {
        Self {
            kind,
            url: None,
            commit: None,
            path: Some(path.into()),
        }
    }
}

/// A theme format motif can read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SourceKind {
    /// An Omarchy theme's `colors.toml`.
    OmarchyColors,
    /// An Omarchy theme's legacy `alacritty.toml`.
    OmarchyAlacritty,
    /// A Base16 scheme.
    Base16,
    /// A Base24 scheme.
    Base24,
}

impl SourceKind {
    /// Returns a short lowercase name for the format.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OmarchyColors => "omarchy-colors",
            Self::OmarchyAlacritty => "omarchy-alacritty",
            Self::Base16 => "base16",
            Self::Base24 => "base24",
        }
    }
}

impl fmt::Display for SourceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
