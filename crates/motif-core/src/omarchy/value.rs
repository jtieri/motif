//! Colors written in the forms Omarchy themes use.

use crate::Color;

/// The error returned when an Omarchy value is not a color.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ParseValueError {
    /// The value is empty.
    #[error("the value is empty")]
    Empty,
    /// The value is not in any color form motif knows.
    #[error("not a color in a form motif reads (#rrggbb, rgb(), rgba() or a gradient)")]
    Unrecognized,
    /// A `#` color does not have six or eight hex digits.
    #[error("{0:?} is not #rrggbb or #rrggbbaa")]
    Hex(String),
    /// The arguments of `rgb()` or `rgba()` are malformed.
    #[error("{0:?} does not have valid arguments")]
    Function(String),
}

/// Parses an Omarchy color value.
///
/// Accepts the forms found in Omarchy themes, ignoring any alpha:
///
/// - `#rrggbb` and `#rrggbbaa`;
/// - Hyprland's `rgb(rrggbb)`, `rgba(rrggbbaa)` and `rgba(rrggbb)`;
/// - CSS's `rgb(r, g, b)` and `rgba(r, g, b, a)` with channels from 0 to 255
///   and alpha from 0 to 1;
/// - a Hyprland gradient, color stops separated by spaces and an optional
///   final angle such as `45deg`, which gives its first stop.
///
/// Hex digits and function names may be in either case. Leading and trailing
/// spaces are ignored.
///
/// # Errors
///
/// Returns a [`ParseValueError`] describing the first problem found.
///
/// # Examples
///
/// ```
/// use motif_core::Color;
/// use motif_core::omarchy::parse_color;
///
/// let red = Color::new(0xff, 0, 0);
/// assert_eq!(parse_color("#ff0000"), Ok(red));
/// assert_eq!(parse_color("rgba(ff0000ee) rgba(00ff00ee) 45deg"), Ok(red));
/// assert_eq!(parse_color("rgba(255, 0, 0, 0.8)"), Ok(red));
/// ```
pub fn parse_color(value: &str) -> Result<Color, ParseValueError> {
    let tokens = split_stops(value.trim_matches(' '))?;
    match tokens.as_slice() {
        [] => Err(ParseValueError::Empty),
        [single] => parse_stop(single),
        [head @ .., last] => {
            let stops = if is_angle(last) { head } else { &tokens };
            let [first, rest @ ..] = stops else {
                return Err(ParseValueError::Unrecognized);
            };
            let color = parse_stop(first)?;
            for stop in rest {
                parse_stop(stop)?;
            }
            Ok(color)
        }
    }
}

/// Splits on spaces outside parentheses.
fn split_stops(value: &str) -> Result<Vec<&str>, ParseValueError> {
    let mut tokens = Vec::new();
    let mut depth = 0_u32;
    let mut start = None;
    for (i, c) in value.char_indices() {
        match c {
            '(' => depth += 1,
            ')' => depth = depth.checked_sub(1).ok_or(ParseValueError::Unrecognized)?,
            ' ' if depth == 0 => {
                if let Some(s) = start.take() {
                    tokens.push(&value[s..i]);
                }
                continue;
            }
            _ => {}
        }
        start.get_or_insert(i);
    }
    if depth != 0 {
        return Err(ParseValueError::Unrecognized);
    }
    if let Some(s) = start {
        tokens.push(&value[s..]);
    }
    Ok(tokens)
}

fn is_angle(token: &str) -> bool {
    let Some(number) = token.strip_suffix("deg") else {
        return false;
    };
    let number = number.strip_prefix(['-', '+']).unwrap_or(number);
    !number.is_empty()
        && number.parse::<f64>().is_ok()
        && number.bytes().all(|b| b.is_ascii_digit() || b == b'.')
}

fn parse_stop(stop: &str) -> Result<Color, ParseValueError> {
    if let Some(digits) = stop.strip_prefix('#') {
        return hex(digits, true).ok_or_else(|| ParseValueError::Hex(stop.to_owned()));
    }
    let Some(open) = stop.find('(') else {
        return Err(ParseValueError::Unrecognized);
    };
    let name = &stop[..open];
    let alpha = if name.eq_ignore_ascii_case("rgb") {
        false
    } else if name.eq_ignore_ascii_case("rgba") {
        true
    } else {
        return Err(ParseValueError::Unrecognized);
    };
    let args = stop[open + 1..]
        .strip_suffix(')')
        .ok_or(ParseValueError::Unrecognized)?;
    function_args(args, alpha).ok_or_else(|| ParseValueError::Function(stop.to_owned()))
}

/// Parses six hex digits, or eight when `alpha` allows it.
fn hex(digits: &str, alpha: bool) -> Option<Color> {
    if !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    match digits.len() {
        6 => Color::from_hex6(digits),
        8 if alpha => Color::from_hex6(&digits[..6]),
        _ => None,
    }
}

fn function_args(args: &str, alpha: bool) -> Option<Color> {
    let parts: Vec<&str> = args.split(',').map(|p| p.trim_matches(' ')).collect();
    match (parts.as_slice(), alpha) {
        ([digits], _) => hex(digits, alpha),
        ([r, g, b], false) => Some(Color::new(channel(r)?, channel(g)?, channel(b)?)),
        ([r, g, b, a], true) => {
            is_alpha(a).then_some(())?;
            Some(Color::new(channel(r)?, channel(g)?, channel(b)?))
        }
        _ => None,
    }
}

fn channel(s: &str) -> Option<u8> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    s.parse().ok()
}

fn is_alpha(s: &str) -> bool {
    !s.is_empty()
        && s.bytes().all(|b| b.is_ascii_digit() || b == b'.')
        && s.parse::<f64>().is_ok_and(|a| (0.0..=1.0).contains(&a))
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    const RED: Color = Color::new(0xff, 0, 0);

    #[test]
    fn reads_every_form_in_the_catalog() {
        for value in [
            "#ff0000",
            "#FF0000",
            "#ff000080",
            " #ff0000 ",
            "rgb(ff0000)",
            "rgba(ff0000ee)",
            "rgba(ff0000)",
            "rgba(255,0,0,0.8)",
            "rgba(255, 0, 0, 1)",
            "rgb(255, 0, 0)",
            "RGBA(FF0000EE)",
            "rgba(ff0000ee) rgba(00ff00ee) 45deg",
            "rgba(ff0000ee) rgba(00ff00ee)",
            "rgb(ff0000) rgb(00ff00) rgb(0000ff) 25deg",
            "#ff0000 #00ff00 -45deg",
            "#ff0000  #00ff00  35.5deg",
            "rgba(255, 0, 0, 0.5) #00ff00 0deg",
        ] {
            assert_eq!(parse_color(value), Ok(RED), "{value}");
        }
    }

    #[test]
    fn rejects_malformed_values() {
        for (value, err) in [
            ("", ParseValueError::Empty),
            ("   ", ParseValueError::Empty),
            ("45deg", ParseValueError::Unrecognized),
            ("cyan blue 45deg", ParseValueError::Unrecognized),
            ("#4A8B8Bi", ParseValueError::Hex("#4A8B8Bi".into())),
            ("#fff", ParseValueError::Hex("#fff".into())),
            ("#ÿÿÿÿ", ParseValueError::Hex("#ÿÿÿÿ".into())),
            (
                "rgb(ff0000ee)",
                ParseValueError::Function("rgb(ff0000ee)".into()),
            ),
            (
                "rgb(256,0,0)",
                ParseValueError::Function("rgb(256,0,0)".into()),
            ),
            (
                "rgb(+1,0,0)",
                ParseValueError::Function("rgb(+1,0,0)".into()),
            ),
            (
                "rgba(1,2,3,1.5)",
                ParseValueError::Function("rgba(1,2,3,1.5)".into()),
            ),
            (
                "rgba(1,2,3)",
                ParseValueError::Function("rgba(1,2,3)".into()),
            ),
            ("rgb(1,2,3", ParseValueError::Unrecognized),
            ("rgb)1(", ParseValueError::Unrecognized),
            ("hsl(1,2,3)", ParseValueError::Unrecognized),
            ("#ff0000 45deg 45deg", ParseValueError::Unrecognized),
            ("#ff0000 nope", ParseValueError::Unrecognized),
            ("dark", ParseValueError::Unrecognized),
            ("0.5", ParseValueError::Unrecognized),
        ] {
            assert_eq!(parse_color(value), Err(err), "{value}");
        }
    }

    proptest! {
        #[test]
        fn never_panics(value in "\\PC*") {
            let _ = parse_color(&value);
        }

        #[test]
        fn never_panics_on_color_like_text(value in "[#rgba(),. 0-9a-fA-Fdeg+-]{0,40}") {
            let _ = parse_color(&value);
        }

        #[test]
        fn reads_any_hex(r: u8, g: u8, b: u8, a: u8, upper: bool) {
            let color = Color::new(r, g, b);
            let mut hex = format!("{r:02x}{g:02x}{b:02x}");
            let mut hexa = format!("{hex}{a:02x}");
            if upper {
                hex.make_ascii_uppercase();
                hexa.make_ascii_uppercase();
            }
            prop_assert_eq!(parse_color(&format!("#{hex}")), Ok(color));
            prop_assert_eq!(parse_color(&format!("#{hexa}")), Ok(color));
            prop_assert_eq!(parse_color(&format!("rgb({hex})")), Ok(color));
            prop_assert_eq!(parse_color(&format!("rgba({hexa})")), Ok(color));
            prop_assert_eq!(parse_color(&format!("rgb({r}, {g}, {b})")), Ok(color));
        }

        #[test]
        fn reads_the_first_stop_of_a_gradient(
            stops in prop::collection::vec(any::<(u8, u8, u8)>(), 1..8),
            angle in prop::option::of(0_u16..360),
        ) {
            let mut value = stops
                .iter()
                .map(|(r, g, b)| format!("rgba({r:02x}{g:02x}{b:02x}ee)"))
                .collect::<Vec<_>>()
                .join(" ");
            if let Some(angle) = angle {
                value = format!("{value} {angle}deg");
            }
            let (r, g, b) = stops[0];
            prop_assert_eq!(parse_color(&value), Ok(Color::new(r, g, b)));
        }

        #[test]
        fn display_round_trips(r: u8, g: u8, b: u8) {
            let color = Color::new(r, g, b);
            prop_assert_eq!(parse_color(&color.to_string()), Ok(color));
            prop_assert_eq!(color.to_string().parse::<Color>(), Ok(color));
        }
    }
}
