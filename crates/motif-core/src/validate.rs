//! Checking a palette file against the dotfiles' palette schema.
//!
//! [`validate`] reads any palette document, curated or imported. Structural
//! problems make the palette unusable: theme-apply or a template would fail
//! on it. Low contrast does not; imported themes keep the colors their
//! authors chose, so contrast is reported as warnings for the caller to show.

use std::fmt;

use serde_json::{Map, Value};

use crate::contrast::{NON_TEXT, TEXT, contrast_ratio};
use crate::palette::{ANSI_KEYS, ROLES};
use crate::{Color, Slug};

/// The names in `meta.scheme_names` that every palette sets.
const SCHEME_NAMES: [&str; 8] = [
    "nvim_colorscheme",
    "zed_dark_theme",
    "zed_light_theme",
    "jetbrains_scheme",
    "gtk_theme",
    "gtk_icon_theme",
    "papirus_folder_color",
    "cursor_theme",
];

/// The pairs whose contrast is checked: a foreground role, the role it is
/// drawn on, and the least contrast expected.
///
/// Text roles are held to [`TEXT`]. Colors that mark state or decorate,
/// such as `primary` on the surface or an outline, and the text roles meant
/// for low emphasis, such as the dimmest text tier (used for comments and
/// hints) and `on_*_fixed_variant`, are held to [`NON_TEXT`].
pub const PAIRS: [(&str, &str, f64); 37] = [
    ("on_surface", "surface", TEXT),
    ("on_surface", "surface_container_lowest", TEXT),
    ("on_surface", "surface_container_low", TEXT),
    ("on_surface", "surface_container", TEXT),
    ("on_surface", "surface_container_high", TEXT),
    ("on_surface", "surface_container_highest", TEXT),
    ("on_background", "background", TEXT),
    ("on_surface_variant", "surface", TEXT),
    ("on_surface_variant", "surface_container", TEXT),
    ("inverse_on_surface", "inverse_surface", TEXT),
    ("on_primary", "primary", TEXT),
    ("on_secondary", "secondary", TEXT),
    ("on_tertiary", "tertiary", TEXT),
    ("on_error", "error", TEXT),
    ("on_primary_container", "primary_container", TEXT),
    ("on_secondary_container", "secondary_container", TEXT),
    ("on_tertiary_container", "tertiary_container", TEXT),
    ("on_error_container", "error_container", TEXT),
    ("on_primary_fixed", "primary_fixed", TEXT),
    ("on_primary_fixed_variant", "primary_fixed", NON_TEXT),
    ("on_secondary_fixed", "secondary_fixed", TEXT),
    ("on_secondary_fixed_variant", "secondary_fixed", NON_TEXT),
    ("on_tertiary_fixed", "tertiary_fixed", TEXT),
    ("on_tertiary_fixed_variant", "tertiary_fixed", NON_TEXT),
    ("on_surface_dim", "surface", NON_TEXT),
    ("outline", "surface", NON_TEXT),
    ("primary", "surface", NON_TEXT),
    ("secondary", "surface", NON_TEXT),
    ("tertiary", "surface", NON_TEXT),
    ("accent", "surface", NON_TEXT),
    ("error", "surface", NON_TEXT),
    ("warning", "surface", NON_TEXT),
    ("success", "surface", NON_TEXT),
    ("primary", "surface_container", NON_TEXT),
    ("error", "surface_container", NON_TEXT),
    ("inverse_primary", "inverse_surface", NON_TEXT),
    ("surface_tint", "surface", NON_TEXT),
];

/// The outcome of checking a palette.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Report {
    /// What makes the palette unusable, in the order found.
    pub problems: Vec<Problem>,
    /// Pairs of roles with less contrast than expected, in [`PAIRS`] order.
    pub warnings: Vec<ContrastWarning>,
}

impl Report {
    /// Reports whether the palette has no structural problems. Contrast
    /// warnings do not count.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        self.problems.is_empty()
    }
}

/// A structural problem in a palette.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Problem {
    /// The key at fault, as a dotted path such as `palette.primary`.
    pub key: String,
    /// What is wrong with it.
    pub message: String,
}

impl fmt::Display for Problem {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.key, self.message)
    }
}

/// A pair of roles with less contrast than expected.
#[derive(Debug, Clone, PartialEq)]
pub struct ContrastWarning {
    /// The role drawn on top, such as `on_primary`.
    pub foreground: &'static str,
    /// The role it is drawn on, such as `primary`.
    pub background: &'static str,
    /// The contrast the palette gives the pair.
    pub ratio: f64,
    /// The least contrast expected of the pair.
    pub minimum: f64,
}

impl fmt::Display for ContrastWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} on {} has contrast {:.2}:1, below {}:1",
            self.foreground, self.background, self.ratio, self.minimum
        )
    }
}

/// Checks a palette document.
///
/// The document must have:
///
/// - a `meta` object with a non-empty `name`, a valid [`Slug`], a `mode` of
///   `dark` or `light`, a `source_color` and a `scheme_names` object naming
///   every theme the switcher applies;
/// - if `meta.accents` is present, an `available` list whose every name is a
///   key of `colors` or `palette`;
/// - an `ansi` object with `color0` to `color15` and `cursor`;
/// - a `palette` object with every key in [`ROLES`];
/// - if `colors` is present, an object of colors.
///
/// Every color is a `#rrggbb`. Hex digits may be in either case, since
/// matugen and theme-apply accept both and palettes the dotfiles derive from
/// a wallpaper can hold uppercase ones; the palettes motif writes are always
/// lowercase. Keys the schema does not name are allowed.
#[must_use]
pub fn validate(doc: &Value) -> Report {
    let mut check = Checker::default();
    let Some(root) = check.object(doc, "the palette") else {
        return check.report;
    };

    let meta = check.member_object(root, "meta", "meta");
    let ansi = check.member_object(root, "ansi", "ansi");
    let palette = check.member_object(root, "palette", "palette");
    let colors = match root.get("colors") {
        Some(value) => check.object(value, "colors"),
        None => None,
    };

    if let Some(meta) = meta {
        check.meta(meta, colors, palette);
    }
    if let Some(ansi) = ansi {
        for key in ANSI_KEYS {
            check.member_color(ansi, key, "ansi");
        }
    }
    if let Some(colors) = colors {
        for (key, value) in colors {
            check.color(value, &format!("colors.{key}"));
        }
    }
    if let Some(palette) = palette {
        let roles: Vec<Option<Color>> = ROLES
            .iter()
            .map(|key| check.member_color(palette, key, "palette"))
            .collect();
        let role = |name: &str| ROLES.iter().position(|&r| r == name).and_then(|i| roles[i]);
        for (foreground, background, minimum) in PAIRS {
            if let (Some(fg), Some(bg)) = (role(foreground), role(background)) {
                let ratio = contrast_ratio(fg, bg);
                if ratio < minimum {
                    check.report.warnings.push(ContrastWarning {
                        foreground,
                        background,
                        ratio,
                        minimum,
                    });
                }
            }
        }
    }
    check.report
}

#[derive(Debug, Default)]
struct Checker {
    report: Report,
}

impl Checker {
    fn problem(&mut self, key: &str, message: impl Into<String>) {
        self.report.problems.push(Problem {
            key: key.to_owned(),
            message: message.into(),
        });
    }

    fn object<'a>(&mut self, value: &'a Value, key: &str) -> Option<&'a Map<String, Value>> {
        let object = value.as_object();
        if object.is_none() {
            self.problem(key, "is not an object");
        }
        object
    }

    fn member_object<'a>(
        &mut self,
        parent: &'a Map<String, Value>,
        name: &str,
        key: &str,
    ) -> Option<&'a Map<String, Value>> {
        if let Some(value) = parent.get(name) {
            self.object(value, key)
        } else {
            self.problem(key, "is missing");
            None
        }
    }

    fn member_str<'a>(
        &mut self,
        parent: &'a Map<String, Value>,
        name: &str,
        key: &str,
    ) -> Option<&'a str> {
        match parent.get(name) {
            Some(Value::String(s)) => Some(s),
            Some(_) => {
                self.problem(key, "is not a string");
                None
            }
            None => {
                self.problem(key, "is missing");
                None
            }
        }
    }

    fn color(&mut self, value: &Value, key: &str) -> Option<Color> {
        // Either case is accepted: matugen and theme-apply read both, and the
        // dotfiles' saved wallpaper palettes keep the uppercase hex wallust
        // writes. Requiring lowercase would reject palettes that work.
        let parsed = value.as_str().and_then(|s| s.parse::<Color>().ok());
        if parsed.is_none() {
            self.problem(key, "is not a #rrggbb color");
        }
        parsed
    }

    fn member_color(
        &mut self,
        parent: &Map<String, Value>,
        name: &str,
        section: &str,
    ) -> Option<Color> {
        let key = format!("{section}.{name}");
        if let Some(value) = parent.get(name) {
            self.color(value, &key)
        } else {
            self.problem(&key, "is missing");
            None
        }
    }

    fn meta(
        &mut self,
        meta: &Map<String, Value>,
        colors: Option<&Map<String, Value>>,
        palette: Option<&Map<String, Value>>,
    ) {
        if self.member_str(meta, "name", "meta.name") == Some("") {
            self.problem("meta.name", "is empty");
        }
        if let Some(Err(err)) = self
            .member_str(meta, "slug", "meta.slug")
            .map(str::parse::<Slug>)
        {
            self.problem("meta.slug", err.kind().to_string());
        }
        if let Some(mode) = self.member_str(meta, "mode", "meta.mode")
            && mode != "dark"
            && mode != "light"
        {
            self.problem("meta.mode", "is neither dark nor light");
        }
        self.member_color(meta, "source_color", "meta");

        if let Some(names) = self.member_object(meta, "scheme_names", "meta.scheme_names") {
            for name in SCHEME_NAMES {
                self.member_str(names, name, &format!("meta.scheme_names.{name}"));
            }
        }

        let Some(accents) = meta.get("accents") else {
            return;
        };
        let Some(accents) = self.object(accents, "meta.accents") else {
            return;
        };
        match accents.get("available") {
            Some(Value::Array(available)) => {
                for (i, name) in available.iter().enumerate() {
                    let key = format!("meta.accents.available[{i}]");
                    let Some(name) = name.as_str() else {
                        self.problem(&key, "is not a string");
                        continue;
                    };
                    let found = [colors, palette]
                        .into_iter()
                        .flatten()
                        .any(|section| section.contains_key(name));
                    if !found {
                        self.problem(&key, format!("{name:?} is in neither colors nor palette"));
                    }
                }
            }
            Some(_) => self.problem("meta.accents.available", "is not a list"),
            None => self.problem("meta.accents.available", "is missing"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn minimal() -> Value {
        let mut palette = Map::new();
        for role in ROLES {
            palette.insert(role.to_owned(), json!("#808080"));
        }
        palette.insert("on_surface".into(), json!("#ffffff"));
        palette.insert("surface".into(), json!("#000000"));
        let mut ansi = Map::new();
        for key in ANSI_KEYS {
            ansi.insert(key.to_owned(), json!("#000000"));
        }
        let names: Map<String, Value> = SCHEME_NAMES
            .iter()
            .map(|n| ((*n).to_owned(), json!("x")))
            .collect();
        json!({
            "meta": {
                "name": "Test",
                "slug": "test",
                "mode": "dark",
                "source_color": "#808080",
                "scheme_names": names,
                "accents": { "available": ["accent_1", "red"] },
            },
            "ansi": ansi,
            "colors": { "red": "#ff0000" },
            "palette": palette,
        })
    }

    fn problems(doc: &Value) -> Vec<String> {
        validate(doc)
            .problems
            .iter()
            .map(ToString::to_string)
            .collect()
    }

    #[test]
    fn accepts_a_complete_palette() {
        let report = validate(&minimal());
        assert_eq!(report.problems, []);
        assert!(report.is_valid());
        // Grey on grey is low in contrast; white on black is not.
        assert_ne!(report.warnings, []);
        assert!(
            report
                .warnings
                .iter()
                .all(|w| (w.foreground, w.background) != ("on_surface", "surface"))
        );
    }

    #[test]
    fn reports_missing_and_malformed_keys() {
        let mut doc = minimal();
        doc["palette"].as_object_mut().unwrap().remove("accent_10");
        doc["palette"]["primary"] = json!("#ABCDEF");
        doc["palette"]["secondary"] = json!("abcdef");
        doc["ansi"]["cursor"] = json!(7);
        doc["meta"]["slug"] = json!("wallpaper-derived");
        doc["meta"]["mode"] = json!("dim");
        doc["meta"]["accents"]["available"] = json!(["teal"]);
        doc["colors"]["blue"] = json!("#00f");
        assert_eq!(
            problems(&doc),
            [
                "meta.slug: it is reserved for the wallpaper-derived palette",
                "meta.mode: is neither dark nor light",
                "meta.accents.available[0]: \"teal\" is in neither colors nor palette",
                "ansi.cursor: is not a #rrggbb color",
                "colors.blue: is not a #rrggbb color",
                "palette.secondary: is not a #rrggbb color",
                "palette.accent_10: is missing",
            ]
        );
    }

    #[test]
    fn reports_missing_sections() {
        assert_eq!(problems(&json!([])), ["the palette: is not an object"]);
        assert_eq!(
            problems(&json!({ "palette": 1 })),
            [
                "meta: is missing",
                "ansi: is missing",
                "palette: is not an object"
            ]
        );
    }

    #[test]
    fn describes_contrast_warnings() {
        let warning = ContrastWarning {
            foreground: "on_primary",
            background: "primary",
            ratio: 3.214,
            minimum: TEXT,
        };
        assert_eq!(
            warning.to_string(),
            "on_primary on primary has contrast 3.21:1, below 4.5:1"
        );
    }
}
