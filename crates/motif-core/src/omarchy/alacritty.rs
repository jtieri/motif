//! The converter of `omarchy-theme-colors-from-alacritty`, ported byte for
//! byte.
//!
//! Older Omarchy themes have no `colors.toml`, only an `alacritty.toml`.
//! Omarchy generates the missing `colors.toml` from it and then reads that
//! like any other. The script does not parse TOML: an awk pass reads
//! `key = value` lines under `[section]` headers and keeps the values that are
//! a lone six-digit hex color, then bash reads awk's output into an
//! associative array and writes the file. This module reproduces both halves,
//! including how bash's `read` splits awk's output on tabs.
//!
//! gawk in a UTF-8 locale treats bytes that are not UTF-8 in two ways the
//! port copies. Its `.` and `[^]]` do not match them, so a header or a trailing
//! comment holding them is rejected. And `substr` on a line that also holds a
//! valid multibyte character turns each of them into U+FFFD, which `.` does
//! match. The two disagree on what is valid: the matcher accepts encoded
//! surrogates and rejects code points past U+10FFFF, while glibc, which
//! `substr` relies on, does the opposite and also accepts the old five- and
//! six-byte forms.

use std::borrow::Cow;
use std::collections::HashMap;
use std::ops::RangeInclusive;

/// The terminal color names in ANSI order, as the script lists them.
const NAMES: [&str; 8] = [
    "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
];

/// The error returned when an `alacritty.toml` lacks a normal color, in which
/// case Omarchy writes no `colors.toml`.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("missing {}", .keys.join(", "))]
pub struct MissingColors {
    keys: Vec<&'static str>,
}

impl MissingColors {
    /// Returns the missing keys in ANSI order, such as `colors.normal.black`.
    #[must_use]
    pub fn keys(&self) -> &[&'static str] {
        &self.keys
    }
}

/// Converts the contents of an `alacritty.toml` into the `colors.toml` that
/// `omarchy-theme-colors-from-alacritty` writes for it.
///
/// The output is the script's, byte for byte. Its values are `#rrggbb`
/// colors except in files with tabs inside headers or keys, where bash's
/// splitting can carry other text into a value as it does in Omarchy.
///
/// When a key is set both under `[colors.normal]` and as `normal.black`
/// under `[colors]`, the header form wins, and otherwise the first valid
/// occurrence of a key wins. The script's result depends on gawk's hash
/// order in one case: two keys that bash reads as the same key because of a
/// tab or a NUL byte in them. There the port takes awk to print the keys
/// written under headers in file order and then the dotted keys, so the last
/// of them wins.
///
/// # Errors
///
/// Returns [`MissingColors`] if any of the eight normal colors has no valid
/// value, as Omarchy then generates nothing.
///
/// # Examples
///
/// ```
/// use motif_core::omarchy::colors_from_alacritty;
///
/// let alacritty = b"[colors.normal]\nblack = '0x1D2021'\n";
/// let err = colors_from_alacritty(alacritty).unwrap_err();
/// assert_eq!(err.keys()[0], "colors.normal.red");
/// ```
pub fn colors_from_alacritty(bytes: &[u8]) -> Result<Vec<u8>, MissingColors> {
    let colors = read_colors(bytes);
    let get = |section: &str, name: &str| -> &[u8] {
        let key = format!("colors.{section}.{name}");
        colors.get(key.as_bytes()).map_or(&[], Vec::as_slice)
    };

    let mut ansi: [&[u8]; 16] = [&[]; 16];
    let mut missing = Vec::new();
    for (i, name) in NAMES.iter().enumerate() {
        ansi[i] = get("normal", name);
        if ansi[i].is_empty() {
            missing.push(NORMAL_KEYS[i]);
        }
    }
    if !missing.is_empty() {
        return Err(MissingColors { keys: missing });
    }
    for (i, name) in NAMES.iter().enumerate() {
        ansi[i + 8] = or(get("bright", name), ansi[i]);
    }

    let background = or(get("primary", "background"), ansi[0]);
    let foreground = or(get("primary", "foreground"), ansi[7]);
    ansi[0] = background;
    ansi[7] = foreground;
    let selection = or(get("selection", "background"), foreground);

    let mut out = Vec::new();
    push_line(&mut out, "accent", ansi[4]);
    push_line(&mut out, "selection", selection);
    out.push(b'\n');
    push_line(&mut out, "background", background);
    push_line(&mut out, "foreground", foreground);
    out.push(b'\n');
    for (i, value) in ansi.iter().enumerate() {
        push_line(&mut out, &format!("color{i}"), value);
    }
    Ok(out)
}

/// The keys of the normal colors, as [`MissingColors`] names them.
const NORMAL_KEYS: [&str; 8] = [
    "colors.normal.black",
    "colors.normal.red",
    "colors.normal.green",
    "colors.normal.yellow",
    "colors.normal.blue",
    "colors.normal.magenta",
    "colors.normal.cyan",
    "colors.normal.white",
];

/// `${a:-b}`.
fn or<'a>(value: &'a [u8], fallback: &'a [u8]) -> &'a [u8] {
    if value.is_empty() { fallback } else { value }
}

/// Writes `key = "value"` as the script's here-document does, inserting the
/// value as it is.
fn push_line(out: &mut Vec<u8>, key: &str, value: &[u8]) {
    out.extend_from_slice(key.as_bytes());
    out.extend_from_slice(b" = \"");
    out.extend_from_slice(value);
    out.extend_from_slice(b"\"\n");
}

/// Runs the awk pass and bash's loop over its output, returning the
/// associative array bash builds: dotted paths such as `colors.normal.black`
/// mapped to values such as `#1d2021`.
fn read_colors(bytes: &[u8]) -> HashMap<Vec<u8>, Vec<u8>> {
    let (direct, dotted) = awk_pass(bytes);
    let mut colors = HashMap::new();
    let printed = direct
        .iter()
        .chain(dotted.iter().filter(|(path, _)| !direct.contains_key(path)));
    for (path, hex) in printed {
        // awk prints `path\t#hex`; bash drops NUL bytes and splits on tabs.
        let mut line: Vec<u8> = path.iter().copied().filter(|&b| b != 0).collect();
        line.extend_from_slice(b"\t#");
        line.extend_from_slice(hex);
        let (key, value) = split_on_tabs(&line);
        colors.insert(key.to_vec(), value.to_vec());
    }
    colors
}

/// Pairs of dotted paths and lowercase hex digits in the order they first
/// appear in the file, looked up by path.
#[derive(Debug, Default)]
struct Ordered {
    order: Vec<Vec<u8>>,
    hex: HashMap<Vec<u8>, [u8; 6]>,
}

impl Ordered {
    /// Keeps the first value given for a path, as `if (!(path in a))` does.
    fn insert_first(&mut self, path: Vec<u8>, hex: [u8; 6]) {
        if !self.hex.contains_key(&path) {
            self.hex.insert(path.clone(), hex);
            self.order.push(path);
        }
    }

    fn contains_key(&self, path: &[u8]) -> bool {
        self.hex.contains_key(path)
    }

    fn iter(&self) -> impl Iterator<Item = (&[u8], &[u8; 6])> {
        self.order
            .iter()
            .map(|path| (path.as_slice(), &self.hex[path]))
    }
}

/// The awk program: collects `section.key` paths with a color value, those
/// written under a header and those written as dotted keys under `[colors]`
/// separately.
fn awk_pass(bytes: &[u8]) -> (Ordered, Ordered) {
    let mut direct = Ordered::default();
    let mut dotted = Ordered::default();
    let mut section = Vec::new();

    // awk reads a last line that lacks its newline too.
    let mut records: Vec<&[u8]> = bytes.split(|&b| b == b'\n').collect();
    if bytes.is_empty() || bytes.ends_with(b"\n") {
        records.pop();
    }

    for record in records {
        let line = trim_start(record);
        if line.first() == Some(&b'[') {
            section = header(line).map_or_else(Vec::new, |name| substr(record, name).into_owned());
            continue;
        }
        if section.is_empty() {
            continue;
        }
        let Some(eq) = record.iter().position(|&b| b == b'=') else {
            continue;
        };
        let key = substr(record, &record[..eq]);
        let key = trim(&key);
        if key.is_empty() || key[0] == b'#' {
            continue;
        }
        let Some(hex) = color_value(trim(&substr(record, &record[eq + 1..]))) else {
            continue;
        };

        let mut path = section.clone();
        path.push(b'.');
        path.extend_from_slice(key);
        if section == b"colors" && key.contains(&b'.') {
            dotted.insert_first(path, hex);
        } else {
            direct.insert_first(path, hex);
        }
    }
    (direct, dotted)
}

/// Returns `part` of `record` as gawk's `substr` returns it: unchanged,
/// unless `record` holds a multibyte character, in which case every byte
/// glibc cannot decode becomes U+FFFD.
fn substr<'a>(record: &[u8], part: &'a [u8]) -> Cow<'a, [u8]> {
    if !has_multibyte(record) {
        return Cow::Borrowed(part);
    }
    let mut out = Vec::with_capacity(part.len());
    let mut i = 0;
    while i < part.len() {
        if let Some(len) = char_len(&part[i..], glibc_lead) {
            out.extend_from_slice(&part[i..i + len]);
            i += len;
        } else {
            out.extend_from_slice("\u{fffd}".as_bytes());
            i += 1;
        }
    }
    Cow::Owned(out)
}

/// Returns the name inside a header line that matches `^\[[^]]*\][ \t]*$`.
fn header(line: &[u8]) -> Option<&[u8]> {
    let inner = line.strip_prefix(b"[")?;
    let close = inner.iter().position(|&b| b == b']')?;
    let (name, rest) = (&inner[..close], &inner[close + 1..]);
    (rest.iter().all(|&b| is_blank(b)) && matches_dot(name)).then_some(name)
}

/// Returns the six hex digits of a value matching either of the script's
/// patterns, lowercased:
///
/// ```text
/// ^["'](0[xX]|#)?[0-9a-fA-F]{6}(["']([ \t]*#.*)?)?$
/// ^(0[xX])?[0-9a-fA-F]{6}([ \t]*#.*)?$
/// ```
fn color_value(value: &[u8]) -> Option<[u8; 6]> {
    let is_quote = |b: &u8| matches!(b, b'"' | b'\'');
    let (quoted, rest) = match value.split_first() {
        Some((first, rest)) if is_quote(first) => (true, rest),
        _ => (false, value),
    };
    let rest = rest
        .strip_prefix(b"0x")
        .or_else(|| rest.strip_prefix(b"0X"))
        .or_else(|| rest.strip_prefix(b"#").filter(|_| quoted))
        .unwrap_or(rest);
    if rest.len() < 6 || !rest[..6].iter().all(u8::is_ascii_hexdigit) {
        return None;
    }
    let (digits, tail) = rest.split_at(6);
    let tail_matches = match tail.split_first() {
        None => true,
        Some((close, comment)) if quoted && is_quote(close) => {
            comment.is_empty() || is_comment(comment)
        }
        Some(_) => !quoted && is_comment(tail),
    };
    if !tail_matches {
        return None;
    }
    let mut hex = [0; 6];
    hex.copy_from_slice(digits);
    Some(hex.map(|b| b.to_ascii_lowercase()))
}

/// Reports whether `s` matches `^[ \t]*#.*$`.
fn is_comment(s: &[u8]) -> bool {
    trim_start(s).strip_prefix(b"#").is_some_and(matches_dot)
}

/// Splits a line the way `IFS=$'\t' read -r key value` does: tabs around the
/// line are dropped, the key ends at the first tab and the value is the rest
/// after the run of tabs that follows it.
fn split_on_tabs(line: &[u8]) -> (&[u8], &[u8]) {
    let is_tab = |b: &u8| *b == b'\t';
    let start = line.iter().position(|b| !is_tab(b)).unwrap_or(line.len());
    let line = &line[start..];
    let end = line.iter().rposition(|b| !is_tab(b)).map_or(0, |i| i + 1);
    let line = &line[..end];
    match line.iter().position(is_tab) {
        None => (line, &[]),
        Some(tab) => {
            let rest = &line[tab..];
            let value = rest.iter().position(|b| !is_tab(b)).unwrap_or(rest.len());
            (&line[..tab], &rest[value..])
        }
    }
}

fn is_blank(b: u8) -> bool {
    matches!(b, b' ' | b'\t')
}

fn trim_start(s: &[u8]) -> &[u8] {
    let start = s.iter().position(|&b| !is_blank(b)).unwrap_or(s.len());
    &s[start..]
}

fn trim(s: &[u8]) -> &[u8] {
    let s = trim_start(s);
    let end = s.iter().rposition(|&b| !is_blank(b)).map_or(0, |i| i + 1);
    &s[..end]
}

/// Describes the sequences a UTF-8 decoder accepts: given a sequence's first
/// byte, the sequence's length and the range of its second byte. Any further
/// bytes are in `0x80..=0xbf`.
type Lead = fn(u8) -> Option<(usize, RangeInclusive<u8>)>;

/// The sequences gawk's `.` matches: UTF-8 as Rust checks it, plus the
/// encoded surrogates U+D800 to U+DFFF.
fn regex_lead(b: u8) -> Option<(usize, RangeInclusive<u8>)> {
    Some(match b {
        0x00..=0x7f => (1, 0x80..=0xbf),
        0xc2..=0xdf => (2, 0x80..=0xbf),
        0xe0 => (3, 0xa0..=0xbf),
        0xe1..=0xef => (3, 0x80..=0xbf),
        0xf0 => (4, 0x90..=0xbf),
        0xf1..=0xf3 => (4, 0x80..=0xbf),
        0xf4 => (4, 0x80..=0x8f),
        _ => return None,
    })
}

/// The sequences glibc decodes: every code point up to U+7FFFFFFF in its
/// shortest form, except the surrogates.
fn glibc_lead(b: u8) -> Option<(usize, RangeInclusive<u8>)> {
    Some(match b {
        0x00..=0x7f => (1, 0x80..=0xbf),
        0xc2..=0xdf => (2, 0x80..=0xbf),
        0xe0 => (3, 0xa0..=0xbf),
        0xed => (3, 0x80..=0x9f),
        0xe1..=0xef => (3, 0x80..=0xbf),
        0xf0 => (4, 0x90..=0xbf),
        0xf1..=0xf7 => (4, 0x80..=0xbf),
        0xf8 => (5, 0x88..=0xbf),
        0xf9..=0xfb => (5, 0x80..=0xbf),
        0xfc => (6, 0x84..=0xbf),
        0xfd => (6, 0x80..=0xbf),
        _ => return None,
    })
}

/// Returns the length of the character `s` starts with, or `None` if `lead`
/// rejects it.
fn char_len(s: &[u8], lead: Lead) -> Option<usize> {
    let (len, second) = lead(*s.first()?)?;
    if len > 1 {
        let tail = s.get(1..len)?;
        if !second.contains(&tail[0]) || !tail[1..].iter().all(|b| (0x80..=0xbf).contains(b)) {
            return None;
        }
    }
    Some(len)
}

/// Reports whether `.` matches every character of `s`.
fn matches_dot(mut s: &[u8]) -> bool {
    while !s.is_empty() {
        let Some(len) = char_len(s, regex_lead) else {
            return false;
        };
        s = &s[len..];
    }
    true
}

/// Reports whether glibc decodes a character of more than one byte in `s`.
fn has_multibyte(s: &[u8]) -> bool {
    let mut i = 0;
    while i < s.len() {
        match char_len(&s[i..], glibc_lead) {
            Some(1) | None => i += 1,
            Some(_) => return true,
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::omarchy::ColorsToml;
    use crate::{Color, Provenance, SourceKind};
    use proptest::prelude::*;
    use std::fmt::Write as _;

    const NORMAL: &str = "\
[colors.normal]
black = '#000000'
red = '#110000'
green = '#002200'
yellow = '#333300'
blue = '#000044'
magenta = '#550055'
cyan = '#006666'
white = '#777777'
";

    fn convert(text: &str) -> String {
        String::from_utf8(colors_from_alacritty(text.as_bytes()).unwrap()).unwrap()
    }

    /// Returns the value the generated file gives `key`.
    fn value(text: &str, key: &str) -> String {
        let colors = convert(text);
        let prefix = format!("{key} = \"");
        colors
            .lines()
            .find_map(|line| line.strip_prefix(&prefix)?.strip_suffix('"'))
            .unwrap()
            .to_owned()
    }

    #[test]
    fn writes_the_scripts_file() {
        let text = format!(
            "{NORMAL}[colors.bright]\nred = '#ff0000'\n[colors.primary]\n\
             background = '#0a0a0a'\nforeground = '#f0f0f0'\n\
             [colors.selection]\nbackground = '#123456'\n"
        );
        assert_eq!(
            convert(&text),
            "\
accent = \"#000044\"
selection = \"#123456\"

background = \"#0a0a0a\"
foreground = \"#f0f0f0\"

color0 = \"#0a0a0a\"
color1 = \"#110000\"
color2 = \"#002200\"
color3 = \"#333300\"
color4 = \"#000044\"
color5 = \"#550055\"
color6 = \"#006666\"
color7 = \"#f0f0f0\"
color8 = \"#000000\"
color9 = \"#ff0000\"
color10 = \"#002200\"
color11 = \"#333300\"
color12 = \"#000044\"
color13 = \"#550055\"
color14 = \"#006666\"
color15 = \"#777777\"
"
        );
    }

    #[test]
    fn falls_back_like_the_script() {
        assert_eq!(value(NORMAL, "background"), "#000000");
        assert_eq!(value(NORMAL, "foreground"), "#777777");
        assert_eq!(value(NORMAL, "selection"), "#777777");
        assert_eq!(value(NORMAL, "color9"), "#110000");
    }

    #[test]
    fn reports_missing_normal_colors() {
        let text = NORMAL
            .replace("red = '#110000'\n", "")
            .replace("white", "#white");
        let err = colors_from_alacritty(text.as_bytes()).unwrap_err();
        assert_eq!(err.keys(), ["colors.normal.red", "colors.normal.white"]);
        assert_eq!(
            err.to_string(),
            "missing colors.normal.red, colors.normal.white"
        );
        assert_eq!(colors_from_alacritty(b"").unwrap_err().keys().len(), 8);
    }

    #[test]
    fn reads_every_value_form() {
        for (written, hex) in [
            ("'#AbCdEf'", "#abcdef"),
            ("\"#abcdef\"", "#abcdef"),
            ("\"0xabcdef\"", "#abcdef"),
            ("'0Xabcdef'", "#abcdef"),
            ("0xabcdef", "#abcdef"),
            ("abcdef", "#abcdef"),
            ("abcdef# note", "#abcdef"),
            ("\"#abcdef\" # note", "#abcdef"),
            ("\"#abcdef\"#", "#abcdef"),
            ("\"#abcdef", "#abcdef"),
            ("'#abcdef\"", "#abcdef"),
        ] {
            let text = NORMAL.replace("'#000044'", written);
            assert_eq!(value(&text, "color4"), hex, "{written}");
        }
    }

    #[test]
    fn rejects_other_values() {
        for value in [
            "#abcdef",
            "'#abc'",
            "'#abcdef0'",
            "'#abcdef80'",
            "' #abcdef'",
            "'#abcdef' x",
            "'0x#abcdef'",
            "x0abcdef",
            "'rgb(1,2,3)'",
            "abcdef x",
        ] {
            let text = NORMAL.replace("'#000044'", value);
            assert!(colors_from_alacritty(text.as_bytes()).is_err(), "{value}");
        }
        let invalid = NORMAL.replace("'#000044'", "'#000044' # \u{e9}");
        let mut bytes = invalid.into_bytes();
        let at = bytes.iter().position(|&b| b == 0xa9).unwrap();
        bytes.remove(at);
        assert!(colors_from_alacritty(&bytes).is_err());
    }

    #[test]
    fn prefers_headers_to_dotted_keys() {
        let text = format!("[colors]\nnormal.blue = '#ffffff'\n{NORMAL}");
        assert_eq!(value(&text, "color4"), "#000044");
        let dotted: String = NORMAL
            .lines()
            .map(|line| match line {
                "[colors.normal]" => "[colors]\n".to_owned(),
                _ => format!("normal.{line}\n"),
            })
            .collect();
        assert_eq!(value(&dotted, "color4"), "#000044");
    }

    #[test]
    fn keeps_the_first_valid_value() {
        let text = format!("{NORMAL}blue = '#ffffff'\n")
            .replace("blue = '#000044'", "blue = x\nblue = '#000044'");
        assert_eq!(value(&text, "color4"), "#000044");
    }

    #[test]
    fn reads_headers_like_the_script() {
        let indented = NORMAL.replace("[colors.normal]", " \t[colors.normal] \t");
        assert_eq!(value(&indented, "color1"), "#110000");
        for header in [
            "[colors.normal] # note",
            "[ colors.normal ]",
            "[[colors.normal]]",
            "[colors.normal]\r",
        ] {
            let text = NORMAL.replace("[colors.normal]", header);
            assert!(colors_from_alacritty(text.as_bytes()).is_err(), "{header}");
        }
        // Through bash's splitting, this header sets red unless awk rejects it.
        let without_red = NORMAL.replace("red = '#110000'\n", "");
        let header = format!("{without_red}[colors.normal.red\t\u{e9}]\nk = '#ffffff'\n");
        assert_eq!(value(&header, "color1"), "\u{e9}.k\t#ffffff");
        let mut invalid = header.into_bytes();
        invalid.retain(|&b| b != 0xa9);
        assert!(colors_from_alacritty(&invalid).is_err());
        let reset = format!("{NORMAL}[colors.normal] x\nblue = '#ffffff'\n");
        assert_eq!(value(&reset, "color4"), "#000044");
        assert!(colors_from_alacritty(b"black = '#000000'\n").is_err());
    }

    #[test]
    fn reads_a_last_line_without_a_newline() {
        let text = NORMAL.trim_end();
        assert_eq!(value(text, "color7"), "#777777");
    }

    #[test]
    fn splits_awk_output_like_bash() {
        assert_eq!(
            split_on_tabs(b"a.b\t#123456"),
            (&b"a.b"[..], &b"#123456"[..])
        );
        assert_eq!(
            split_on_tabs(b"\t\ta\t\tb\t#1\t\t"),
            (&b"a"[..], &b"b\t#1"[..])
        );
        assert_eq!(split_on_tabs(b"a"), (&b"a"[..], &b""[..]));
        // A tab in a header or key changes the key bash sees.
        let leading = NORMAL.replace("[colors.normal]", "[\tcolors.normal]");
        assert_eq!(value(&leading, "color1"), "#110000");
        let inner = NORMAL.replace("red = ", "red\tx = ");
        assert_eq!(value(&inner, "color1"), "x\t#110000");
        let nul = NORMAL.replace("red = ", "r\0ed = ");
        assert_eq!(value(&nul, "color1"), "#110000");
    }

    #[test]
    fn matches_dot_like_gawk() {
        assert!(matches_dot(b""));
        assert!(matches_dot("a\0é\u{fffe}\u{10ffff}".as_bytes()));
        assert!(matches_dot(b"\xed\xa0\x80"));
        for bad in [
            &b"\x80"[..],
            b"\xc0\x80",
            b"\xc1\xbf",
            b"\xe0\x80\x80",
            b"\xf0\x80\x80\x80",
            b"\xf4\x90\x80\x80",
            b"\xf5\x80\x80\x80",
            b"\xe2\x80",
            b"\xff",
        ] {
            assert!(!matches_dot(bad), "{bad:?}");
        }
    }

    #[test]
    fn substr_replaces_bytes_like_gawk() {
        assert_eq!(substr(b"k\xff=v", b"k\xff"), &b"k\xff"[..]);
        assert_eq!(substr(b"k\xff=\xc3\xa9", b"k\xff"), "k\u{fffd}".as_bytes());
        assert_eq!(
            substr("\u{e9}=".as_bytes(), b"\xed\xa0\x80\xe1\x80"),
            "\u{fffd}".repeat(5).as_bytes()
        );
        assert_eq!(
            substr(b"\xf4\x90\x80\x80", b"\xf4\x90\x80\x80\xff"),
            &b"\xf4\x90\x80\x80\xef\xbf\xbd"[..]
        );
        assert!(has_multibyte(b"\xfc\x84\x80\x80\x80\x80"));
        assert!(!has_multibyte(b"\xed\xa0\x80\xc0\x80\xfe"));
        // A comment `.` cannot match becomes one it can.
        let text = NORMAL.replace("'#000044'", "'#000044' # \u{e9}\u{80}");
        let mut bytes = text.into_bytes();
        let at = bytes.windows(2).position(|w| w == b"\xc2\x80").unwrap();
        bytes.remove(at);
        let colors = colors_from_alacritty(&bytes).unwrap();
        assert!(colors.windows(18).any(|w| w == b"color4 = \"#000044\""));
    }

    /// Writes `color` in one of the forms the script accepts.
    fn written(color: Color, form: u8) -> String {
        let hex = format!("{:02x}{:02x}{:02x}", color.r, color.g, color.b);
        match form % 6 {
            0 => format!("'#{hex}'"),
            1 => format!("\"#{}\"", hex.to_uppercase()),
            2 => format!("'0x{hex}'"),
            3 => format!("0x{hex}"),
            4 => format!("{hex} # note"),
            _ => format!("\"0X{hex}\"  # note"),
        }
    }

    fn color() -> impl Strategy<Value = Color> {
        any::<(u8, u8, u8)>().prop_map(|(r, g, b)| Color::new(r, g, b))
    }

    proptest! {
        #[test]
        fn never_panics(bytes in prop::collection::vec(any::<u8>(), 0..512)) {
            let _ = colors_from_alacritty(&bytes);
        }

        #[test]
        fn never_panics_on_alacritty_like_text(
            text in "([\\[\\]a-z.= \t\"'#0-9A-Fx\n]|\u{80}|\u{e9}){0,200}",
        ) {
            let _ = colors_from_alacritty(text.as_bytes());
        }

        #[test]
        fn converts_what_it_is_given(
            normal in prop::array::uniform8(color()),
            bright in prop::array::uniform8(prop::option::of(color())),
            primary in prop::option::of((color(), color())),
            selection in prop::option::of(color()),
            forms in prop::array::uniform16(any::<u8>()),
            dotted: bool,
        ) {
            let mut text = String::new();
            let mut group = |section: &str, pairs: Vec<(&str, Color, u8)>| {
                let dotted = dotted && section != "primary";
                let header = if dotted { "colors" } else { &format!("colors.{section}") };
                writeln!(text, "[{header}]").unwrap();
                for (name, color, form) in pairs {
                    let prefix = if dotted { format!("{section}.") } else { String::new() };
                    writeln!(text, "{prefix}{name} = {}", written(color, form)).unwrap();
                }
            };
            let normal_pairs = NAMES
                .iter()
                .zip(normal)
                .zip(forms)
                .map(|((name, color), form)| (*name, color, form))
                .collect();
            group("normal", normal_pairs);
            let bright_pairs = NAMES
                .iter()
                .zip(bright)
                .zip(&forms[8..])
                .filter_map(|((name, color), form)| Some((*name, color?, *form)))
                .collect();
            group("bright", bright_pairs);
            if let Some((bg, fg)) = primary {
                group("primary", vec![("background", bg, forms[0]), ("foreground", fg, forms[1])]);
            }
            if let Some(sel) = selection {
                group("selection", vec![("background", sel, forms[2])]);
            }

            let colors = colors_from_alacritty(text.as_bytes()).unwrap();
            let parsed = ColorsToml::parse(&colors);
            prop_assert!(parsed.skipped().is_empty());
            let theme = parsed
                .resolve(false)
                .to_theme(String::new(), Provenance::local(SourceKind::OmarchyAlacritty, "x"))
                .unwrap();

            let (bg, fg) = primary.unwrap_or((normal[0], normal[7]));
            let mut ansi = [Color::BLACK; 16];
            for i in 0..8 {
                ansi[i] = normal[i];
                ansi[i + 8] = bright[i].unwrap_or(normal[i]);
            }
            ansi[0] = bg;
            ansi[7] = fg;
            prop_assert_eq!(theme.ansi, ansi);
            prop_assert_eq!(theme.background.base, bg);
            prop_assert_eq!(theme.foreground.base, fg);
            prop_assert_eq!(theme.accent, Some(normal[4]));
            prop_assert_eq!(theme.selection.background, Some(selection.unwrap_or(fg)));
        }
    }
}
