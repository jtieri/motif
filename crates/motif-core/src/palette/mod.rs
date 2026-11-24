//! Turning a [`Theme`] into a dotfiles palette.
//!
//! A palette is the JSON file matugen renders the desktop's templates from,
//! described in the dotfiles' `palettes/_schema.md`. [`Palette::from_theme`]
//! fills one from a theme the way that document's recipe fills a curated
//! palette by hand: the colors a theme publishes are used as they are, and
//! the tones it does not publish come from matugen's color system.
//! `docs/mapping.md` gives the source of every key.

mod harvest;
mod hls;
mod hues;
mod semantic;

use std::collections::HashMap;

use serde::ser::{Serialize, SerializeMap, Serializer};

pub use semantic::{ParseSemanticError, Semantic};

use self::harvest::{Tones, harvest, readable_on};
use self::hls::blend;
use self::semantic::Status;
use crate::{Color, Mode, NamedColor, Slug, SlugError, Theme};

/// The keys of a palette's `palette` section, in the order palette files
/// list them: the Material You roles, the four extensions (`warning`,
/// `success`, `accent`, `on_surface_dim`) and the ten accent slots.
pub const ROLES: [&str; 63] = [
    "surface_container_lowest",
    "surface_container_low",
    "surface",
    "surface_container",
    "surface_container_high",
    "surface_container_highest",
    "surface_dim",
    "surface_bright",
    "surface_variant",
    "background",
    "surface_tint",
    "on_surface",
    "on_surface_variant",
    "on_surface_dim",
    "on_background",
    "primary",
    "on_primary",
    "primary_container",
    "on_primary_container",
    "secondary",
    "on_secondary",
    "secondary_container",
    "on_secondary_container",
    "tertiary",
    "on_tertiary",
    "tertiary_container",
    "on_tertiary_container",
    "error",
    "on_error",
    "error_container",
    "on_error_container",
    "primary_fixed",
    "primary_fixed_dim",
    "on_primary_fixed",
    "on_primary_fixed_variant",
    "secondary_fixed",
    "secondary_fixed_dim",
    "on_secondary_fixed",
    "on_secondary_fixed_variant",
    "tertiary_fixed",
    "tertiary_fixed_dim",
    "on_tertiary_fixed",
    "on_tertiary_fixed_variant",
    "warning",
    "success",
    "accent",
    "outline",
    "outline_variant",
    "inverse_surface",
    "inverse_on_surface",
    "inverse_primary",
    "scrim",
    "shadow",
    "accent_1",
    "accent_2",
    "accent_3",
    "accent_4",
    "accent_5",
    "accent_6",
    "accent_7",
    "accent_8",
    "accent_9",
    "accent_10",
];

/// The keys of a palette's `ansi` section, in order.
pub const ANSI_KEYS: [&str; 17] = [
    "color0", "color1", "color2", "color3", "color4", "color5", "color6", "color7", "color8",
    "color9", "color10", "color11", "color12", "color13", "color14", "color15", "cursor",
];

/// The names given to ANSI colors 1 to 6 when a theme names no hues.
const ANSI_HUE_NAMES: [&str; 6] = ["red", "green", "yellow", "blue", "magenta", "cyan"];

/// Choices that a theme does not settle.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Options {
    /// The palette's display name, instead of the theme's.
    pub name: Option<String>,
    /// The palette's slug, instead of one made from its name.
    pub slug: Option<Slug>,
    /// How the status colors are chosen.
    pub semantic: Semantic,
}

/// A dotfiles palette.
///
/// Serializes to the palette file format, with every section and key in the
/// order the dotfiles' own palettes use.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Palette {
    /// The palette's identity and the names of themes to pair it with.
    pub meta: Meta,
    /// The 16 terminal colors and the cursor.
    pub ansi: ColorMap,
    /// The theme's hues under its own names, which the accent picker offers.
    pub colors: ColorMap,
    /// The Material You roles, extensions and accent slots, in [`ROLES`]
    /// order.
    pub palette: ColorMap,
}

/// A palette's `meta` section.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Meta {
    /// The name shown in the theme picker.
    pub name: String,
    /// The file stem and state key.
    pub slug: Slug,
    /// Whether the palette is dark or light.
    #[serde(serialize_with = "display")]
    pub mode: Mode,
    /// The color matugen seeds its scheme with when the palette is applied.
    pub source_color: Color,
    /// The themes paired with the palette for what matugen does not render.
    pub scheme_names: SchemeNames,
    /// The accents the accent picker offers.
    pub accents: Accents,
    /// Where the palette's theme came from.
    pub source: Source,
}

/// The names of installed themes that a palette selects.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
#[allow(missing_docs)]
pub struct SchemeNames {
    pub nvim_colorscheme: String,
    pub zed_dark_theme: String,
    pub zed_light_theme: String,
    pub jetbrains_scheme: String,
    pub gtk_theme: String,
    pub gtk_icon_theme: String,
    pub papirus_folder_color: String,
    pub cursor_theme: String,
}

/// The accent picker's settings.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Accents {
    /// The GTK theme to use with any accent.
    pub gtk_theme: String,
    /// The Papirus folder color; `{accent_hex}` stands for the accent.
    pub papirus_folder_color: String,
    /// The names in `colors` that can be picked.
    pub available: Vec<String>,
}

/// Where a palette's theme came from.
///
/// The theme's local path is left out: palettes are synced to every machine,
/// where a path from one of them means nothing.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct Source {
    /// The format the theme was read from, such as `omarchy-colors`.
    pub kind: String,
    /// The repository or page the theme was fetched from.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The commit the theme was read at.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
}

/// Named colors in a fixed order, serialized as a JSON object.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ColorMap(Vec<(String, Color)>);

impl ColorMap {
    /// Returns the color under `key`.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<Color> {
        self.0.iter().find(|(k, _)| k == key).map(|&(_, c)| c)
    }

    /// Returns the keys and colors in order.
    pub fn iter(&self) -> impl Iterator<Item = (&str, Color)> {
        self.0.iter().map(|(k, c)| (k.as_str(), *c))
    }

    /// Returns the number of colors.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Reports whether there are no colors.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Adds `color` under `key` unless the key is already present.
    fn insert_new(&mut self, key: &str, color: Color) {
        if self.get(key).is_none() {
            self.0.push((key.to_owned(), color));
        }
    }
}

impl Serialize for ColorMap {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for (key, color) in &self.0 {
            map.serialize_entry(key, color)?;
        }
        map.end()
    }
}

fn display<T: std::fmt::Display, S: Serializer>(
    value: &T,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    serializer.collect_str(value)
}

impl Palette {
    /// Builds a palette from a theme.
    ///
    /// # Errors
    ///
    /// Returns a [`SlugError`] if `options` gives no slug and none can be
    /// made from the name, as with a name of only non-ASCII letters.
    pub fn from_theme(theme: &Theme, options: &Options) -> Result<Self, SlugError> {
        let name = options.name.clone().unwrap_or_else(|| theme.name.clone());
        let slug = match &options.slug {
            Some(slug) => slug.clone(),
            None => Slug::from_name(&name)?,
        };
        let colors = hue_names(theme);
        let palette = roles(theme, &colors, options.semantic);
        let accent = palette.get("primary").unwrap_or(theme.ansi[4]);

        let mut ansi = ColorMap::default();
        for (key, &color) in ANSI_KEYS.iter().zip(&theme.ansi) {
            ansi.insert_new(key, color);
        }
        ansi.insert_new("cursor", theme.cursor.unwrap_or(theme.foreground.base));

        let dark = theme.mode == Mode::Dark;
        let gtk_theme = if dark { "adw-gtk3-dark" } else { "adw-gtk3" };
        let meta = Meta {
            name,
            slug,
            mode: theme.mode,
            source_color: accent,
            scheme_names: SchemeNames {
                nvim_colorscheme: "dotfiles-base16".to_owned(),
                zed_dark_theme: "Dotfiles".to_owned(),
                zed_light_theme: "Dotfiles".to_owned(),
                jetbrains_scheme: "Catppuccin Mocha".to_owned(),
                gtk_theme: gtk_theme.to_owned(),
                gtk_icon_theme: if dark {
                    "Papirus-Dark"
                } else {
                    "Papirus-Light"
                }
                .to_owned(),
                papirus_folder_color: accent.to_string(),
                cursor_theme: if dark {
                    "capitaine-cursors"
                } else {
                    "capitaine-cursors-light"
                }
                .to_owned(),
            },
            accents: Accents {
                gtk_theme: gtk_theme.to_owned(),
                papirus_folder_color: "{accent_hex}".to_owned(),
                available: colors.iter().map(|(k, _)| k.to_owned()).collect(),
            },
            source: Source {
                kind: theme.provenance.kind.as_str().to_owned(),
                url: theme.provenance.url.clone(),
                commit: theme.provenance.commit.clone(),
            },
        };

        Ok(Self {
            meta,
            ansi,
            colors,
            palette,
        })
    }

    /// Checks the palette as [`validate`](crate::validate::validate) checks a
    /// palette file.
    #[must_use]
    #[allow(clippy::missing_panics_doc, reason = "a palette always serializes")]
    pub fn validate(&self) -> crate::validate::Report {
        crate::validate::validate(&serde_json::to_value(self).expect("a palette always serializes"))
    }

    /// Returns the palette as pretty-printed JSON with two-space indents and
    /// a final newline, as the dotfiles' palette files are written.
    #[must_use]
    #[allow(clippy::missing_panics_doc, reason = "a palette always serializes")]
    pub fn to_json(&self) -> String {
        // A palette holds only strings and string-keyed maps, which always
        // serialize.
        let mut json = serde_json::to_string_pretty(self).expect("a palette always serializes");
        json.push('\n');
        json
    }
}

/// Returns the theme's hues under its names, or ANSI colors 1 to 6 under the
/// usual color names when it names none. A repeated name keeps its first
/// color.
fn hue_names(theme: &Theme) -> ColorMap {
    let mut colors = ColorMap::default();
    if theme.hues.is_empty() {
        for (name, &color) in ANSI_HUE_NAMES.iter().zip(&theme.ansi[1..7]) {
            colors.insert_new(name, color);
        }
    } else {
        for NamedColor { name, color } in &theme.hues {
            colors.insert_new(name, *color);
        }
    }
    colors
}

/// Fills every key of the `palette` section.
fn roles(theme: &Theme, hues: &ColorMap, semantic: Semantic) -> ColorMap {
    let mode = theme.mode;
    let bg = theme.background.base;
    let fg = theme.foreground.base;
    let ansi = &theme.ansi;

    // Tiers a source leaves out are filled as build_base16 in
    // derive-palette.py fills the slots they map to.
    let lift = |t| blend(bg, fg, t);
    let away = match mode {
        Mode::Dark => Color::BLACK,
        Mode::Light => Color::WHITE,
    };
    let bg_dark = theme.background.dark.unwrap_or_else(|| lift(0.03));
    let bg_darker = theme
        .background
        .darker
        .unwrap_or_else(|| blend(bg, away, 0.30));
    let bg_lighter = theme.background.lighter.unwrap_or_else(|| lift(0.05));
    let fg_light = theme
        .foreground
        .light
        .unwrap_or_else(|| blend(fg, bg, 0.28));
    let fg_dark = theme.foreground.dark.unwrap_or_else(|| blend(fg, bg, 0.45));
    let muted = theme.muted.unwrap_or_else(|| blend(fg, bg, 0.55));
    let primary = theme.accent.unwrap_or_else(|| {
        let candidates: Vec<Color> = ansi[1..7].iter().chain(&ansi[9..15]).copied().collect();
        hues::most_saturated(&candidates).unwrap_or(ansi[4])
    });

    // Raised surfaces continue from the theme's raised background toward
    // its text, in steps about the size Catppuccin uses.
    let high = blend(bg_lighter, fg, 0.12);
    let highest = blend(bg_lighter, fg, 0.24);

    let named = |name: &str, index: usize| hues.get(name).unwrap_or(ansi[index]);
    let status = Status::new(
        semantic,
        mode,
        named("red", 1),
        named("yellow", 3),
        named("green", 2),
    );
    let pool: Vec<Color> = hues.iter().map(|(_, color)| color).collect();
    let (secondary, tertiary) = hues::secondary_and_tertiary(
        primary,
        bg,
        &pool,
        &[status.error, status.warning, status.success],
    );

    let mut roles: HashMap<String, Color> = HashMap::new();
    let mut set = |key: &str, color: Color| {
        roles.insert(key.to_owned(), color);
    };
    set("background", bg);
    set("surface", bg);
    set("surface_dim", bg_dark);
    set("surface_container_low", bg_dark);
    set("surface_container_lowest", bg_darker);
    set("surface_container", bg_lighter);
    set("surface_container_high", high);
    set("surface_container_highest", highest);
    set("surface_bright", highest);
    set("surface_variant", bg_lighter);
    set("outline_variant", bg_lighter);
    set("on_surface", fg);
    set("on_background", fg);
    set("inverse_surface", fg);
    set("on_surface_variant", fg_light);
    set("on_surface_dim", fg_dark);
    set("outline", muted);
    set("inverse_on_surface", bg_dark);
    set("primary", primary);
    set("surface_tint", primary);
    set("accent", primary);
    set("secondary", secondary);
    set("tertiary", tertiary);
    set("error", status.error);
    set("warning", status.warning);
    set("success", status.success);
    set("scrim", Color::BLACK);
    set("shadow", Color::BLACK);

    let tones = |seed| readable_tones(seed, mode);
    let primary_tones = tones(primary);
    set("inverse_primary", primary_tones.inverse);
    for (role, tones) in [
        ("primary", primary_tones),
        ("secondary", tones(secondary)),
        ("tertiary", tones(tertiary)),
    ] {
        set_tones(&mut roles, role, tones, true);
    }
    set_tones(&mut roles, "error", tones(status.error), false);

    let accents = hues::accents(&pool, ansi, mode);
    for (key, color) in ROLES[ROLES.len() - hues::ACCENTS..].iter().zip(accents) {
        roles.insert((*key).to_owned(), color);
    }

    let mut out = ColorMap::default();
    for key in ROLES {
        // Every role is set above; a missing one is a bug the tests catch.
        if let Some(&color) = roles.get(key) {
            out.insert_new(key, color);
        }
    }
    out
}

/// Returns matugen's tones for `seed`, with the text tone made to read on
/// `seed` itself: the roles hold the theme's own colors rather than
/// matugen's re-toned ones.
fn readable_tones(seed: Color, mode: Mode) -> Tones {
    let tones = harvest(seed, mode);
    Tones {
        on_role: readable_on(seed, mode, tones.on_role),
        ..tones
    }
}

/// Sets the derived tones of `role`: `on_<role>`, `<role>_container` and
/// `on_<role>_container`, and with `fixed` the four `_fixed` keys.
fn set_tones(roles: &mut HashMap<String, Color>, role: &str, tones: Tones, fixed: bool) {
    let mut set = |key: String, color: Color| {
        roles.insert(key, color);
    };
    set(format!("on_{role}"), tones.on_role);
    set(format!("{role}_container"), tones.container);
    set(format!("on_{role}_container"), tones.on_container);
    if fixed {
        set(format!("{role}_fixed"), tones.fixed);
        set(format!("{role}_fixed_dim"), tones.fixed_dim);
        set(format!("on_{role}_fixed"), tones.on_fixed);
        set(format!("on_{role}_fixed_variant"), tones.on_fixed_variant);
    }
}
