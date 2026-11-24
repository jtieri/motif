//! Turning a resolved Omarchy palette into a [`Theme`].

use std::error::Error;
use std::fmt;

use super::resolve::{ANSI, LEGACY_PALETTE};
use super::{ParseValueError, Resolved, parse_color};
use crate::Color;
use crate::theme::{Background, Foreground, Mode, NamedColor, Provenance, Selection, Theme};

/// Omarchy's hue names in the order its own themes list them. `orange`,
/// `purple` and `brown` count as the theme's hues only when its file sets
/// them, since Omarchy otherwise copies or darkens another hue.
const HUES: [&str; 9] = [
    "red", "yellow", "orange", "green", "cyan", "blue", "magenta", "purple", "brown",
];
const OPTIONAL_HUES: [&str; 3] = ["orange", "purple", "brown"];

/// Keys with a meaning in Omarchy's palette besides the ones in the tables
/// shared with the resolver. Every other key holding a color is an extra.
const OTHER_KNOWN: [&str; 15] = [
    "accent",
    "cursor",
    "muted",
    "selection",
    "selection_background",
    "selection_foreground",
    "bright_red",
    "bright_yellow",
    "bright_green",
    "bright_cyan",
    "bright_blue",
    "bright_magenta",
    "bright_purple",
    "mode",
    "theme_type",
];

/// The error returned when a resolved palette cannot form a [`Theme`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThemeError {
    key: String,
    value: String,
    in_file: bool,
    kind: ThemeErrorKind,
}

/// What is wrong with a key of an Omarchy palette.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum ThemeErrorKind {
    /// Neither the key nor any key Omarchy falls back to has a value.
    Missing,
    /// The value is not a color.
    Invalid(ParseValueError),
}

impl ThemeError {
    /// Returns the key at fault.
    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Returns the key's resolved value.
    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }

    /// Reports whether the file sets the key itself, rather than Omarchy
    /// deriving it from other keys.
    #[must_use]
    pub fn in_file(&self) -> bool {
        self.in_file
    }

    /// Returns what is wrong with the key.
    #[must_use]
    pub fn kind(&self) -> &ThemeErrorKind {
        &self.kind
    }
}

impl fmt::Display for ThemeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let key = &self.key;
        let value = &self.value;
        match (&self.kind, self.in_file) {
            (ThemeErrorKind::Missing, _) => write!(f, "{key} has no value"),
            (ThemeErrorKind::Invalid(_), true) => write!(f, "{key} = \"{value}\" is not a color"),
            (ThemeErrorKind::Invalid(_), false) => write!(
                f,
                "{key} is derived from other keys as \"{value}\", which is not a color"
            ),
        }
    }
}

impl Error for ThemeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match &self.kind {
            ThemeErrorKind::Missing => None,
            ThemeErrorKind::Invalid(err) => Some(err),
        }
    }
}

impl Resolved {
    /// Builds a [`Theme`] from the palette.
    ///
    /// Every key the model takes from Omarchy's palette must hold a color.
    /// The hues are Omarchy's own hue names that the theme provides. Other
    /// keys of the file whose values are colors, which themes use for
    /// application chrome such as window borders, become extras in file
    /// order; keys whose values are not colors are ignored.
    ///
    /// # Errors
    ///
    /// Returns a [`ThemeError`] naming the first key that has no value or a
    /// value that is not a color.
    pub fn to_theme(&self, name: String, provenance: Provenance) -> Result<Theme, ThemeError> {
        let mut ansi = [Color::BLACK; 16];
        let background = Background {
            base: self.color("background")?,
            dark: Some(self.color("dark_background")?),
            darker: Some(self.color("darker_background")?),
            lighter: Some(self.color("lighter_background")?),
        };
        let foreground = Foreground {
            base: self.color("foreground")?,
            dark: Some(self.color("dark_foreground")?),
            light: Some(self.color("light_foreground")?),
            bright: Some(self.color("bright_foreground")?),
        };
        for (slot, (key, _)) in ansi.iter_mut().zip(ANSI) {
            *slot = self.color(key)?;
        }

        Ok(Theme {
            name,
            mode: if self.get("mode") == Some("light") {
                Mode::Light
            } else {
                Mode::Dark
            },
            ansi,
            background,
            foreground,
            accent: Some(self.color("accent")?),
            selection: Selection {
                foreground: Some(self.color("selection_foreground")?),
                background: Some(self.color("selection_background")?),
            },
            muted: Some(self.color("muted")?),
            cursor: Some(self.color("cursor")?),
            hues: self.hues()?,
            extras: self.extras(),
            provenance,
        })
    }

    fn color(&self, key: &str) -> Result<Color, ThemeError> {
        let value = self.get(key).unwrap_or("");
        let error = |kind| ThemeError {
            key: key.to_owned(),
            value: value.to_owned(),
            in_file: self.sets(key),
            kind,
        };
        if value.is_empty() {
            return Err(error(ThemeErrorKind::Missing));
        }
        parse_color(value).map_err(|err| error(ThemeErrorKind::Invalid(err)))
    }

    fn sets(&self, key: &str) -> bool {
        self.file_keys.iter().any(|k| k == key)
    }

    fn hues(&self) -> Result<Vec<NamedColor>, ThemeError> {
        let mut hues = Vec::new();
        for name in HUES {
            if OPTIONAL_HUES.contains(&name) && !self.sets(name) {
                continue;
            }
            if name == "purple" && self.get("purple") == self.get("magenta") {
                continue;
            }
            hues.push(NamedColor {
                name: name.to_owned(),
                color: self.color(name)?,
            });
        }
        Ok(hues)
    }

    fn extras(&self) -> Vec<NamedColor> {
        self.file_keys
            .iter()
            .filter(|key| !is_known(key))
            .filter_map(|key| {
                let color = parse_color(self.get(key)?).ok()?;
                Some(NamedColor {
                    name: key.clone(),
                    color,
                })
            })
            .collect()
    }
}

fn is_known(key: &str) -> bool {
    HUES.contains(&key)
        || OTHER_KNOWN.contains(&key)
        || LEGACY_PALETTE
            .iter()
            .any(|&(k, short)| key == k || key == short)
        || ANSI.iter().any(|&(k, _)| key == k)
}

#[cfg(test)]
mod tests {
    use super::super::ColorsToml;
    use super::*;
    use crate::SourceKind;

    fn theme(text: &str) -> Result<Theme, ThemeError> {
        ColorsToml::parse(text.as_bytes()).resolve(false).to_theme(
            "Test".into(),
            Provenance::local(SourceKind::OmarchyColors, "x"),
        )
    }

    const ANSI_ONLY: &str = "\
color0 = \"#000000\"
color1 = \"#110000\"
color2 = \"#002200\"
color3 = \"#333300\"
color4 = \"#000044\"
color5 = \"#550055\"
color6 = \"#006666\"
color7 = \"#777777\"
color8 = \"#888888\"
color9 = \"#990000\"
color10 = \"#00aa00\"
color11 = \"#bbbb00\"
color12 = \"#0000cc\"
color13 = \"#dd00dd\"
color14 = \"#00eeee\"
color15 = \"#ffffff\"
";

    #[test]
    fn builds_a_theme_from_ansi_colors_alone() {
        let theme = theme(ANSI_ONLY).unwrap();
        assert_eq!(theme.mode, Mode::Dark);
        assert_eq!(theme.ansi[4], Color::new(0, 0, 0x44));
        assert_eq!(theme.background.base, Color::BLACK);
        assert_eq!(theme.foreground.bright, Some(Color::WHITE));
        assert_eq!(theme.accent, Some(Color::new(0, 0, 0x44)));
        assert_eq!(theme.cursor, Some(Color::WHITE));
        let names: Vec<_> = theme.hues.iter().map(|h| h.name.as_str()).collect();
        assert_eq!(names, ["red", "yellow", "green", "cyan", "blue", "magenta"]);
        assert_eq!(theme.extras, []);
    }

    #[test]
    fn separates_hues_from_extras() {
        let text = format!(
            "{ANSI_ONLY}icon_mark = \"#8839ef\"\nborder = \"rgba(112233ee) rgba(445566ee) 45deg\"\n\
             size = \"6\"\norange = \"#ff8800\"\npurple = \"#550055\"\n"
        );
        let theme = theme(&text).unwrap();
        let hues: Vec<_> = theme
            .hues
            .iter()
            .map(|h| (h.name.as_str(), h.color.to_string()))
            .collect();
        assert_eq!(
            &hues[..],
            [
                ("red", "#110000".to_owned()),
                ("yellow", "#333300".into()),
                ("orange", "#ff8800".into()),
                ("green", "#002200".into()),
                ("cyan", "#006666".into()),
                ("blue", "#000044".into()),
                ("magenta", "#550055".into()),
            ]
        );
        let extras: Vec<_> = theme
            .extras
            .iter()
            .map(|h| (h.name.as_str(), h.color.to_string()))
            .collect();
        assert_eq!(
            &extras[..],
            [
                ("icon_mark", "#8839ef".to_owned()),
                ("border", "#112233".into()),
            ]
        );
    }

    #[test]
    fn reports_a_missing_key() {
        let err = theme("mode = \"dark\"\n").unwrap_err();
        assert_eq!(err.key(), "background");
        assert_eq!(err.kind(), &ThemeErrorKind::Missing);
        assert_eq!(err.to_string(), "background has no value");
    }

    #[test]
    fn reports_an_invalid_key_from_the_file() {
        let err = theme(&ANSI_ONLY.replace("\"#000044\"", "\"#4A8B8Bi\"")).unwrap_err();
        assert_eq!(err.key(), "color4");
        assert!(err.in_file());
        assert_eq!(err.to_string(), "color4 = \"#4A8B8Bi\" is not a color");
        assert!(err.source().is_some());
    }

    #[test]
    fn reports_an_invalid_derived_key() {
        let text = ANSI_ONLY.replace("color0 = \"#000000\"", "color0 = \"rgb(000000)\"");
        let err = theme(&text).unwrap_err();
        assert_eq!(err.key(), "dark_background");
        assert!(!err.in_file());
        assert!(err.to_string().starts_with("dark_background is derived"));
    }

    #[test]
    fn reads_the_mode() {
        let light = theme(&format!("{ANSI_ONLY}mode = \"light\"\n")).unwrap();
        assert_eq!(light.mode, Mode::Light);
        let odd = theme(&format!("{ANSI_ONLY}mode = \"Light\"\n")).unwrap();
        assert_eq!(odd.mode, Mode::Dark);
    }
}
