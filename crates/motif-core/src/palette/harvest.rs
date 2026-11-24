//! Tones derived from one color with the HCT color system, as matugen does.
//!
//! A theme publishes the colors of its identity but not the darker and
//! lighter tones that Material You pairs with each of them, such as the text
//! color on a primary button or a muted container. The dotfiles' recipe for
//! curated palettes takes those from `matugen color hex <color>`, which seeds
//! a tonal spot scheme with the color and reads back its primary tones. This
//! module does the same through material-colors, the library matugen is built
//! on, at the version matugen 4.2 uses.
//!
//! [`readable_on`] departs from matugen for one tone: matugen picks the text
//! color for the hue it re-tones, while a palette keeps the theme's own
//! color in the role, so the text color is moved along the same tonal
//! palette until it reads on that color.

use material_colors::color::Argb;
use material_colors::dynamic_color::{DynamicScheme, Variant};
use material_colors::hct::Hct;

use crate::contrast::{TEXT, contrast_ratio};
use crate::{Color, Mode};

/// The tones a tonal spot scheme seeded with one color gives its primary.
///
/// The fields are named after matugen's primary roles; the mapping assigns
/// them to whichever role the seed fills.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Tones {
    pub(crate) on_role: Color,
    pub(crate) container: Color,
    pub(crate) on_container: Color,
    pub(crate) fixed: Color,
    pub(crate) fixed_dim: Color,
    pub(crate) on_fixed: Color,
    pub(crate) on_fixed_variant: Color,
    pub(crate) inverse: Color,
}

/// Derives the tones of `seed` for a theme in `mode`, at matugen's default
/// contrast.
pub(crate) fn harvest(seed: Color, mode: Mode) -> Tones {
    let scheme = scheme(seed, mode);
    Tones {
        on_role: color(scheme.on_primary()),
        container: color(scheme.primary_container()),
        on_container: color(scheme.on_primary_container()),
        fixed: color(scheme.primary_fixed()),
        fixed_dim: color(scheme.primary_fixed_dim()),
        on_fixed: color(scheme.on_primary_fixed()),
        on_fixed_variant: color(scheme.on_primary_fixed_variant()),
        inverse: color(scheme.inverse_primary()),
    }
}

/// Returns a text color for `seed` used as a role's color: `on`, matugen's
/// tone for it, if that has [`TEXT`] contrast against `seed`, and otherwise
/// the first tone of the seed's primary tonal palette that does, stepping
/// one tone at a time from `on`'s tone toward whichever end of the palette
/// (black at 0, white at 100) contrasts more with `seed`. If no tone reaches
/// [`TEXT`], the one with the most contrast is returned; since black or white
/// always gives at least 4.58:1, that does not happen in practice.
pub(crate) fn readable_on(seed: Color, mode: Mode, on: Color) -> Color {
    let ratio = |c: Color| contrast_ratio(c, seed);
    if ratio(on) >= TEXT {
        return on;
    }
    let palette = scheme(seed, mode).primary_palette;
    let tone_color = |tone: i32| color(palette.tone(tone));
    let target = if ratio(tone_color(0)) >= ratio(tone_color(100)) {
        0
    } else {
        100
    };
    let step = if target == 0 { -1 } else { 1 };

    let start = Hct::new(argb(on)).get_tone().round().clamp(0.0, 100.0);
    // The clamp makes the cast exact.
    #[allow(clippy::cast_possible_truncation)]
    let mut tone = start as i32;
    let mut best = on;
    while tone != target {
        tone += step;
        let candidate = tone_color(tone);
        if ratio(candidate) >= TEXT {
            return candidate;
        }
        if ratio(candidate) > ratio(best) {
            best = candidate;
        }
    }
    best
}

/// Seeds matugen's default scheme, tonal spot at standard contrast.
fn scheme(seed: Color, mode: Mode) -> DynamicScheme {
    DynamicScheme::by_variant(argb(seed), &Variant::TonalSpot, mode == Mode::Dark, None)
}

fn argb(color: Color) -> Argb {
    Argb::new(0xff, color.r, color.g, color.b)
}

fn color(argb: Argb) -> Color {
    Color::new(argb.red, argb.green, argb.blue)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Checks every tone against what matugen printed, as recorded by
    /// `scripts/harvest-golden.sh`.
    #[test]
    fn matches_matugen() {
        let expected = include_str!("../../tests/fixtures/harvest/expected.tsv");
        let mut checked = 0;
        for line in expected.lines().filter(|line| !line.starts_with("# ")) {
            let fields: Vec<&str> = line.split('\t').collect();
            let [seed, mode, role, tone] = fields[..] else {
                panic!("malformed line {line:?}");
            };
            let mode = match mode {
                "dark" => Mode::Dark,
                "light" => Mode::Light,
                _ => panic!("unknown mode {mode:?}"),
            };
            let tones = harvest(seed.parse().unwrap(), mode);
            let actual = match role {
                "on_primary" => tones.on_role,
                "primary_container" => tones.container,
                "on_primary_container" => tones.on_container,
                "primary_fixed" => tones.fixed,
                "primary_fixed_dim" => tones.fixed_dim,
                "on_primary_fixed" => tones.on_fixed,
                "on_primary_fixed_variant" => tones.on_fixed_variant,
                "inverse_primary" => tones.inverse,
                _ => panic!("unknown role {role:?}"),
            };
            assert_eq!(actual.to_string(), tone, "{seed} {mode:?} {role}");
            checked += 1;
        }
        assert_eq!(checked, 25 * 2 * 8);
    }

    fn hex(s: &str) -> Color {
        s.parse().unwrap()
    }

    #[test]
    fn keeps_a_readable_matugen_tone() {
        // Catppuccin Mocha's mauve and the tone its curated palette pins.
        let seed = hex("#cba6f7");
        let on = harvest(seed, Mode::Dark).on_role;
        assert_eq!(on, hex("#3c245a"));
        assert_eq!(readable_on(seed, Mode::Dark, on), on);
    }

    #[test]
    fn moves_an_unreadable_tone_along_the_palette() {
        // Lumon's "red", a mid blue, in a dark theme: matugen's dark tone 20
        // gives about 3.3:1 on it.
        let seed = hex("#4d86b0");
        let on = harvest(seed, Mode::Dark).on_role;
        assert!(contrast_ratio(on, seed) < TEXT);
        let readable = readable_on(seed, Mode::Dark, on);
        assert!(contrast_ratio(readable, seed) >= TEXT);
        // It stays in the seed's hue family and darkens, the way it started.
        let palette = scheme(seed, Mode::Dark).primary_palette;
        assert!((0..=100).any(|t| color(palette.tone(t)) == readable));
        assert!(Hct::new(argb(readable)).get_tone() < Hct::new(argb(on)).get_tone());
    }

    #[test]
    fn turns_toward_the_end_that_contrasts_more() {
        // A near-white accent in a light theme: matugen's light on tone is
        // white, which cannot read on it, so the tone moves toward black.
        let seed = hex("#f0f0f0");
        let on = harvest(seed, Mode::Light).on_role;
        let readable = readable_on(seed, Mode::Light, on);
        assert!(contrast_ratio(readable, seed) >= TEXT);
        assert!(Hct::new(argb(readable)).get_tone() < 50.0);
    }

    #[test]
    fn always_reaches_text_contrast() {
        for r in (0..=255).step_by(17) {
            for g in (0..=255).step_by(51) {
                for b in (0..=255).step_by(85) {
                    let seed = Color::new(r, g, b);
                    for mode in [Mode::Dark, Mode::Light] {
                        let on = readable_on(seed, mode, harvest(seed, mode).on_role);
                        assert!(contrast_ratio(on, seed) >= TEXT, "{seed} {mode}");
                    }
                }
            }
        }
    }
}
