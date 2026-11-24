//! Choosing a theme's hues for the palette's roles and accent set.

use material_colors::color::Argb;
use material_colors::hct::Hct;

use super::hls::Hls;
use crate::contrast::{NON_TEXT, contrast_ratio};
use crate::{Color, Mode};

/// The least HCT chroma at which a color reads as a hue rather than a grey.
const MIN_CHROMA: f64 = 16.0;

/// The least difference in HCT hue, in degrees, at which two roles read as
/// different colors.
const MIN_HUE_DISTANCE: f64 = 30.0;

/// The number of accent slots, `accent_1` to `accent_10`.
pub(crate) const ACCENTS: usize = 10;

/// A color with its HCT hue and chroma.
#[derive(Debug, Clone, Copy)]
struct Measured {
    color: Color,
    hue: f64,
    chroma: f64,
}

impl Measured {
    fn new(color: Color) -> Self {
        let hct = Hct::new(Argb::new(0xff, color.r, color.g, color.b));
        Self {
            color,
            hue: hct.get_hue(),
            chroma: hct.get_chroma(),
        }
    }

    fn distance(&self, other: &Self) -> f64 {
        let d = (self.hue - other.hue).rem_euclid(360.0);
        d.min(360.0 - d)
    }
}

/// Chooses the secondary and tertiary colors from `pool`.
///
/// Each is the color nearest in hue to the primary that is still at least
/// [`MIN_HUE_DISTANCE`] from the primary and from the roles already chosen,
/// which gives neighbors on the color wheel the way Catppuccin pairs mauve
/// with blue and teal. Candidates are tried in this order, moving on only
/// when a group is empty:
///
/// 1. chromatic colors, other than the primary and `reserved` (the status
///    colors), with at least [`NON_TEXT`] contrast against `surface`;
/// 2. the same without the contrast requirement;
/// 3. any color that is not the primary.
///
/// When no candidate is far enough from the others, the one farthest from
/// them is taken. Ties go to the color earlier in `pool`. When `pool` holds
/// nothing but the primary, the primary fills both roles.
pub(crate) fn secondary_and_tertiary(
    primary: Color,
    surface: Color,
    pool: &[Color],
    reserved: &[Color],
) -> (Color, Color) {
    let primary = Measured::new(primary);
    let measured: Vec<Measured> = pool
        .iter()
        .filter(|&&c| c != primary.color)
        .map(|&c| Measured::new(c))
        .collect();
    let preferred: Vec<Measured> = measured
        .iter()
        .copied()
        .filter(|m| m.chroma >= MIN_CHROMA && !reserved.contains(&m.color))
        .collect();
    let legible: Vec<Measured> = preferred
        .iter()
        .copied()
        .filter(|m| contrast_ratio(m.color, surface) >= NON_TEXT)
        .collect();

    let mut chosen = vec![primary];
    for _ in 0..2 {
        let candidates: Vec<Measured> = [&legible, &preferred, &measured]
            .into_iter()
            .map(|group| {
                group
                    .iter()
                    .copied()
                    .filter(|m| chosen.iter().all(|c| c.color != m.color))
                    .collect::<Vec<_>>()
            })
            .find(|group| !group.is_empty())
            .unwrap_or_default();
        let next = pick(&candidates, &chosen).unwrap_or(primary);
        chosen.push(next);
    }
    (chosen[1].color, chosen[2].color)
}

/// Picks from `candidates` the color nearest in hue to `chosen[0]` among
/// those at least [`MIN_HUE_DISTANCE`] from every chosen color, or failing
/// that the one whose nearest chosen color is farthest away.
fn pick(candidates: &[Measured], chosen: &[Measured]) -> Option<Measured> {
    let nearest = |m: &Measured| {
        chosen
            .iter()
            .map(|c| m.distance(c))
            .fold(f64::INFINITY, f64::min)
    };
    let distinct = candidates
        .iter()
        .filter(|m| nearest(m) >= MIN_HUE_DISTANCE)
        .min_by(|a, b| a.distance(&chosen[0]).total_cmp(&b.distance(&chosen[0])));
    distinct
        .or_else(|| {
            // max_by keeps the last of equal elements; reverse so the first
            // in pool order wins a tie.
            candidates
                .iter()
                .rev()
                .max_by(|a, b| nearest(a).total_cmp(&nearest(b)))
        })
        .copied()
}

/// Fills the ten accent slots.
///
/// The slots take, in order:
///
/// 1. the theme's hues, skipping any color already taken;
/// 2. the bright ANSI colors 9 to 14 that differ from every color taken;
/// 3. lighter variants (darker for a light theme) of the colors taken, in
///    the order they were taken, stepping HLS lightness by 0.1 and skipping
///    any variant that repeats a color.
///
/// If every variant repeats, which only an all-black or all-white palette can
/// cause, the colors taken are repeated in order.
pub(crate) fn accents(hues: &[Color], ansi: &[Color; 16], mode: Mode) -> [Color; ACCENTS] {
    let mut taken: Vec<Color> = Vec::with_capacity(ACCENTS);
    for &color in hues.iter().chain(&ansi[9..15]) {
        if taken.len() == ACCENTS {
            break;
        }
        if !taken.contains(&color) {
            taken.push(color);
        }
    }
    if taken.is_empty() {
        taken.push(ansi[4]);
    }

    let base = taken.clone();
    let step = match mode {
        Mode::Dark => 0.1,
        Mode::Light => -0.1,
    };
    'steps: for k in 1..=10 {
        for &color in &base {
            if taken.len() == ACCENTS {
                break 'steps;
            }
            let hls = Hls::from_color(color);
            let variant = Hls {
                light: hls.light + step * f64::from(k),
                ..hls
            }
            .to_color();
            if !taken.contains(&variant) {
                taken.push(variant);
            }
        }
    }

    let mut out = [Color::BLACK; ACCENTS];
    for (slot, color) in out.iter_mut().zip(taken.iter().cycle()) {
        *slot = *color;
    }
    out
}

/// Returns the most saturated of `colors` in HLS terms, the first of equals,
/// as `derive-palette.py` picks a primary for a palette that has none.
pub(crate) fn most_saturated(colors: &[Color]) -> Option<Color> {
    colors
        .iter()
        .rev()
        .max_by(|a, b| {
            Hls::from_color(**a)
                .sat
                .total_cmp(&Hls::from_color(**b).sat)
        })
        .copied()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hex(s: &str) -> Color {
        s.parse().unwrap()
    }

    #[test]
    fn picks_neighbors_distinct_from_the_primary() {
        // Catppuccin Mocha as Omarchy ships it: red, yellow, orange, green,
        // cyan, blue, magenta, brown, with blue as the accent.
        let pool = [
            "#f38ba8", "#f9e2af", "#f6b6ab", "#a6e3a1", "#94e2d5", "#89b4fa", "#f5c2e7", "#7b5b55",
        ]
        .map(hex);
        let reserved = [hex("#f38ba8"), hex("#f9e2af"), hex("#a6e3a1")];
        let (secondary, tertiary) =
            secondary_and_tertiary(hex("#89b4fa"), hex("#1e1e2e"), &pool, &reserved);
        // In HCT, blue sits at 262 degrees, pink at 337 and teal at 185.
        assert_eq!(secondary, hex("#f5c2e7"));
        assert_eq!(tertiary, hex("#94e2d5"));
    }

    #[test]
    fn prefers_colors_legible_on_the_surface() {
        // Retro 82's orange accent, its teal and its dark brown.
        let pool = ["#faa968", "#8cbfb8", "#743d1e", "#3f8f8a"].map(hex);
        let (secondary, tertiary) =
            secondary_and_tertiary(hex("#faa968"), hex("#05182e"), &pool, &[]);
        assert_eq!((secondary, tertiary), (hex("#8cbfb8"), hex("#3f8f8a")));
    }

    #[test]
    fn falls_back_to_the_farthest_color() {
        let pool = ["#4d86b0", "#6fa4c9", "#8bc9eb"].map(hex);
        let (secondary, tertiary) =
            secondary_and_tertiary(hex("#8bc9eb"), hex("#16242d"), &pool, &[]);
        assert_ne!(secondary, hex("#8bc9eb"));
        assert_ne!(tertiary, secondary);
    }

    #[test]
    fn uses_the_primary_when_nothing_else_exists() {
        let primary = hex("#888888");
        assert_eq!(
            secondary_and_tertiary(primary, Color::BLACK, &[primary], &[]),
            (primary, primary)
        );
        let other = hex("#123456");
        assert_eq!(
            secondary_and_tertiary(primary, Color::BLACK, &[other], &[]),
            (other, primary)
        );
    }

    #[test]
    fn fills_accents_from_hues_then_bright_ansi_then_variants() {
        let mut ansi = [Color::BLACK; 16];
        ansi[9] = hex("#ff0000");
        ansi[10] = hex("#00ff00");
        let hues = [hex("#ff0000"), hex("#0000ff")];
        let accents = accents(&hues, &ansi, Mode::Dark);
        assert_eq!(
            &accents[..4],
            &[hex("#ff0000"), hex("#0000ff"), hex("#00ff00"), Color::BLACK]
        );
        // Variants of black start at 10% lightness and never repeat a color.
        let mut seen = accents.to_vec();
        seen.sort();
        seen.dedup();
        assert_eq!(seen.len(), ACCENTS);
    }

    #[test]
    fn repeats_when_no_variant_is_new() {
        let ansi = [Color::WHITE; 16];
        let accents = accents(&[Color::WHITE], &ansi, Mode::Dark);
        assert_eq!(accents, [Color::WHITE; ACCENTS]);
    }

    #[test]
    fn picks_the_first_most_saturated() {
        let colors = ["#808080", "#ff0000", "#00ff00", "#7f7f7f"].map(hex);
        assert_eq!(most_saturated(&colors), Some(hex("#ff0000")));
        assert_eq!(most_saturated(&[]), None);
    }
}
