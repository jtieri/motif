//! The alias and fallback cascade of `omarchy-theme-color`, ported step for
//! step.
//!
//! Omarchy keeps every value as text and treats an empty value as unset. The
//! port does the same, including assigning empty strings where the script
//! does, because `--all` prints those keys too. Keep the order of the steps
//! in [`resolve`] identical to the script's.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write as _;

use super::parse::Entry;

/// Canonical background and foreground keys and their legacy short names.
pub(crate) const LEGACY_PALETTE: [(&str, &str); 8] = [
    ("background", "bg"),
    ("dark_background", "dark_bg"),
    ("darker_background", "darker_bg"),
    ("lighter_background", "lighter_bg"),
    ("foreground", "fg"),
    ("dark_foreground", "dark_fg"),
    ("light_foreground", "light_fg"),
    ("bright_foreground", "bright_fg"),
];

/// Semantic hue keys and the ANSI keys they fall back to.
const LEGACY_ANSI: [(&str, &str); 12] = [
    ("red", "color1"),
    ("green", "color2"),
    ("yellow", "color3"),
    ("blue", "color4"),
    ("magenta", "color5"),
    ("cyan", "color6"),
    ("bright_red", "color9"),
    ("bright_green", "color10"),
    ("bright_yellow", "color11"),
    ("bright_blue", "color12"),
    ("bright_magenta", "color13"),
    ("bright_cyan", "color14"),
];

/// The ANSI keys and the semantic keys they fall back to.
pub(crate) const ANSI: [(&str, &str); 16] = [
    ("color0", "background"),
    ("color1", "red"),
    ("color2", "green"),
    ("color3", "yellow"),
    ("color4", "blue"),
    ("color5", "magenta"),
    ("color6", "cyan"),
    ("color7", "foreground"),
    ("color8", "muted"),
    ("color9", "bright_red"),
    ("color10", "bright_green"),
    ("color11", "bright_yellow"),
    ("color12", "bright_blue"),
    ("color13", "bright_magenta"),
    ("color14", "bright_cyan"),
    ("color15", "bright_foreground"),
];

/// The hues brightened when a theme does not give their bright variants.
const BRIGHTENED: [(&str, &str); 6] = [
    ("bright_red", "red"),
    ("bright_yellow", "yellow"),
    ("bright_green", "green"),
    ("bright_cyan", "cyan"),
    ("bright_blue", "blue"),
    ("bright_magenta", "magenta"),
];

/// Omarchy's palette while it resolves: text values, empty meaning unset.
#[derive(Default)]
struct Palette(HashMap<String, String>);

impl Palette {
    fn get(&self, key: &str) -> &str {
        self.0.get(key).map_or("", String::as_str)
    }

    fn is_set(&self, key: &str) -> bool {
        !self.get(key).is_empty()
    }

    fn set(&mut self, key: &str, value: String) {
        self.0.insert(key.to_owned(), value);
    }

    /// `[[ ${C[key]} ]] || C[key]=value`, with `value` computed only when
    /// needed.
    fn set_unless_set(&mut self, key: &str, value: impl FnOnce(&Self) -> String) {
        if !self.is_set(key) {
            let value = value(self);
            self.set(key, value);
        }
    }

    /// `alias_theme_color key fallback`.
    fn alias(&mut self, key: &str, fallback: &str) {
        self.set_unless_set(key, |p| p.get(fallback).to_owned());
    }

    /// `${C[a]:-${C[b]:-...}}`: the first set value of `keys`, or empty.
    fn first(&self, keys: &[&str]) -> String {
        keys.iter()
            .map(|k| self.get(k))
            .find(|v| !v.is_empty())
            .unwrap_or("")
            .to_owned()
    }
}

/// Resolves parsed entries into Omarchy's full palette, keyed and valued
/// exactly as `omarchy-theme-color --all` prints them.
pub(crate) fn resolve(entries: &[Entry], light_mode_file: bool) -> BTreeMap<String, String> {
    let mut c = Palette::default();
    for entry in entries {
        c.set(&entry.key, entry.value.clone());
    }

    for (key, short) in LEGACY_PALETTE {
        c.alias(key, short);
    }

    c.set_unless_set("background", |p| p.get("color0").to_owned());
    c.set_unless_set("foreground", |p| p.get("color7").to_owned());
    if c.is_set("background") {
        c.set("color0", c.get("background").to_owned());
    }
    if c.is_set("foreground") {
        c.set("color7", c.get("foreground").to_owned());
    }

    for (key, ansi) in LEGACY_ANSI {
        c.alias(key, ansi);
    }
    c.alias("magenta", "purple");
    c.alias("bright_magenta", "bright_purple");

    c.set_unless_set("light_foreground", |p| p.first(&["color7", "foreground"]));
    c.set_unless_set("bright_foreground", |p| p.first(&["color15", "foreground"]));
    c.set("cursor", c.get("bright_foreground").to_owned());
    c.set_unless_set("lighter_background", |p| p.first(&["color0", "background"]));
    c.set_unless_set("dark_foreground", |p| p.first(&["color8", "foreground"]));
    c.set_unless_set("muted", |p| p.first(&["color8", "dark_foreground"]));
    c.set_unless_set("selection", |p| {
        p.first(&["selection_background", "color8", "color0", "background"])
    });
    c.alias("selection_background", "selection");
    c.alias("selection_foreground", "bright_foreground");
    c.alias("orange", "yellow");
    c.set_unless_set("brown", |p| mix(p.get("orange"), "#000000", 50));

    c.set_unless_set("dark_background", |p| {
        mix(p.get("background"), "#000000", 25)
    });
    c.set_unless_set("darker_background", |p| {
        mix(p.get("background"), "#000000", 50)
    });
    for (bright, base) in BRIGHTENED {
        c.set_unless_set(bright, |p| mix(p.get(base), "#ffffff", 20));
    }
    c.alias("purple", "magenta");
    c.alias("bright_purple", "bright_magenta");

    for (key, semantic) in ANSI {
        c.alias(key, semantic);
    }
    c.alias("accent", "color4");

    for (key, short) in LEGACY_PALETTE {
        if c.is_set(key) {
            c.set(short, c.get(key).to_owned());
        }
    }

    resolve_mode(&mut c, light_mode_file);
    c.set("theme_type", c.get("mode").to_owned());

    c.0.into_iter().collect()
}

/// `resolve_theme_mode`: the `mode` key, `theme_type`, a `light.mode` file,
/// the background's luminance, then dark.
fn resolve_mode(c: &mut Palette, light_mode_file: bool) {
    c.alias("mode", "theme_type");
    if c.is_set("mode") {
        return;
    }
    let mode = if light_mode_file {
        "light"
    } else if let Some(sum) = channel_sum(c.get("background")) {
        if sum > 382 { "light" } else { "dark" }
    } else {
        "dark"
    };
    c.set("mode", mode.to_owned());
}

/// Sums the channels of a value matching `^#[0-9A-Fa-f]{6}$`.
fn channel_sum(value: &str) -> Option<u32> {
    let color = value.strip_prefix('#').and_then(crate::Color::from_hex6)?;
    Some(u32::from(color.r) + u32::from(color.g) + u32::from(color.b))
}

/// `mix_color start end percent%`, including its behavior on values that are
/// not hex colors.
///
/// The awk original reads each of the first six characters after an optional
/// `#` as a hex digit: a character that is not one counts as -1 and a missing
/// one as 0. The channels are mixed as doubles, rounded by adding 0.5 and
/// truncating toward zero, and a negative result prints as a 64-bit two's
/// complement hex number, as gawk's `%x` does.
fn mix(start: &str, end: &str, percent: u8) -> String {
    let amount = f64::from(percent) / 100.0;
    let start = channels(start);
    let end = channels(end);
    let mut out = String::from("#");
    for (s, e) in start.into_iter().zip(end) {
        #[allow(clippy::cast_possible_truncation)]
        let mixed = (s * (1.0 - amount) + e * amount + 0.5).trunc() as i64;
        if mixed < 0 {
            #[allow(clippy::cast_sign_loss)]
            let _ = write!(out, "{:x}", mixed as u64);
        } else {
            let _ = write!(out, "{mixed:02x}");
        }
    }
    out
}

fn channels(value: &str) -> [f64; 3] {
    let digits = value.strip_prefix('#').unwrap_or(value).as_bytes();
    let digit = |i: usize| -> f64 {
        match digits.get(i) {
            None => 0.0,
            Some(&b) => match (b as char).to_digit(16) {
                Some(d) => f64::from(d),
                None => -1.0,
            },
        }
    };
    [0, 2, 4].map(|i| digit(i) * 16.0 + digit(i + 1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mixes_like_awk() {
        assert_eq!(mix("#282828", "#000000", 25), "#1e1e1e");
        assert_eq!(mix("#ea6962", "#ffffff", 20), "#ee8781");
        assert_eq!(mix("", "#000000", 50), "#000000");
        assert_eq!(mix("", "#ffffff", 20), "#333333");
        assert_eq!(mix("#1", "#ffffff", 20), "#403333");
        assert_eq!(mix("zz", "#000000", 50), "#fffffffffffffff80000");
    }

    #[test]
    fn reads_luminance_only_from_plain_hex() {
        assert_eq!(channel_sum("#7f7f80"), Some(382));
        assert_eq!(channel_sum("#7F7F81"), Some(383));
        assert_eq!(channel_sum("#ffffff80"), None);
        assert_eq!(channel_sum("ffffff"), None);
    }
}
