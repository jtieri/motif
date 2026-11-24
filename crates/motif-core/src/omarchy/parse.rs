//! The line reader of `omarchy-theme-color`, ported byte for byte.
//!
//! Omarchy does not parse `colors.toml` as TOML. It reads `key = value` lines
//! with bash's `read`, cleans them up with parameter expansions and keeps the
//! pairs whose key and value pass two character-set checks. This module
//! reproduces that so motif sees exactly the pairs Omarchy sees.

use std::fmt;

/// One `key = value` pair Omarchy accepts from a `colors.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// The 1-based line number.
    pub line: usize,
    /// The key with quotes and spaces removed.
    pub key: String,
    /// The value with quotes, inline comments and surrounding space removed.
    pub value: String,
}

/// A line Omarchy skips, with a warning, because of unsupported characters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    /// The 1-based line number.
    pub line: usize,
    /// Why the line was skipped.
    pub reason: SkipReason,
}

/// Why a line of a `colors.toml` was skipped.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum SkipReason {
    /// The key has characters other than letters, digits, `_` and `-`.
    Key,
    /// The value has characters Omarchy does not allow.
    Value {
        /// The key the value belongs to.
        key: String,
    },
}

impl fmt::Display for Skipped {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.reason {
            SkipReason::Key => write!(f, "line {}: key has unsupported characters", self.line),
            SkipReason::Value { key } => write!(
                f,
                "line {}: {key} skipped, value has unsupported characters",
                self.line
            ),
        }
    }
}

/// Splits `bytes` into the entries and skipped lines Omarchy would produce.
pub(crate) fn parse(bytes: &[u8]) -> (Vec<Entry>, Vec<Skipped>) {
    let mut entries = Vec::new();
    let mut skipped = Vec::new();

    // `while read ...; done <file` never sees a last line that lacks its
    // newline, so the final segment is dropped.
    let mut lines: Vec<&[u8]> = bytes.split(|&b| b == b'\n').collect();
    lines.pop();

    for (index, raw) in lines.into_iter().enumerate() {
        let line = index + 1;
        // bash drops NUL bytes from input it reads.
        let raw: Vec<u8> = raw.iter().copied().filter(|&b| b != 0).collect();
        let (key, rest) = split_line(&raw);

        let key: Vec<u8> = key
            .iter()
            .copied()
            .filter(|b| !matches!(b, b'"' | b'\'' | b' '))
            .collect();
        if key.is_empty() || key[0] == b'#' {
            continue;
        }

        let value = clean_value(rest);

        if !key.iter().copied().all(is_key_byte) {
            skipped.push(Skipped {
                line,
                reason: SkipReason::Key,
            });
            continue;
        }
        // Both checks admit only ASCII, so the conversions below are lossless.
        let key = String::from_utf8_lossy(&key).into_owned();
        if !value.iter().copied().all(is_value_byte) {
            skipped.push(Skipped {
                line,
                reason: SkipReason::Value { key },
            });
            continue;
        }
        let value = String::from_utf8_lossy(value).into_owned();
        entries.push(Entry { line, key, value });
    }

    (entries, skipped)
}

/// Splits a line the way `IFS='=' read -r key value` does.
fn split_line(line: &[u8]) -> (&[u8], &[u8]) {
    let Some(eq) = line.iter().position(|&b| b == b'=') else {
        return (line, &[]);
    };
    let (key, rest) = (&line[..eq], &line[eq + 1..]);
    // When the rest is a single field followed by one delimiter, `read`
    // removes that delimiter: `a=b=` gives `b`, while `a=b==` keeps `b==`.
    match rest.split_last() {
        Some((b'=', field)) if !field.contains(&b'=') => (key, field),
        _ => (key, rest),
    }
}

/// Takes the text between the first quote and the next one, or trims an
/// unquoted value.
fn clean_value(rest: &[u8]) -> &[u8] {
    let is_quote = |b: &u8| matches!(b, b'"' | b'\'');
    match rest.iter().position(is_quote) {
        Some(open) => {
            let after = &rest[open + 1..];
            let close = after.iter().position(is_quote).unwrap_or(after.len());
            &after[..close]
        }
        None => trim_space(rest),
    }
}

/// The UTF-8 encodings of the characters glibc's `iswspace` accepts, which
/// bash's `[:space:]` matches in a UTF-8 locale.
const SPACES: &[&[u8]] = &[
    b"\t",
    b"\n",
    b"\x0b",
    b"\x0c",
    b"\r",
    b" ",
    "\u{1680}".as_bytes(),
    "\u{2000}".as_bytes(),
    "\u{2001}".as_bytes(),
    "\u{2002}".as_bytes(),
    "\u{2003}".as_bytes(),
    "\u{2004}".as_bytes(),
    "\u{2005}".as_bytes(),
    "\u{2006}".as_bytes(),
    "\u{2008}".as_bytes(),
    "\u{2009}".as_bytes(),
    "\u{200a}".as_bytes(),
    "\u{2028}".as_bytes(),
    "\u{2029}".as_bytes(),
    "\u{205f}".as_bytes(),
    "\u{3000}".as_bytes(),
];

fn trim_space(mut s: &[u8]) -> &[u8] {
    while let Some(rest) = SPACES.iter().find_map(|sp| s.strip_prefix(*sp)) {
        s = rest;
    }
    while let Some(rest) = SPACES.iter().find_map(|sp| s.strip_suffix(*sp)) {
        s = rest;
    }
    s
}

fn is_key_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-')
}

fn is_value_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric()
        || matches!(
            b,
            b'#' | b'(' | b')' | b',' | b'.' | b'_' | b'+' | b'/' | b'%' | b' ' | b'-'
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pairs(input: &str) -> Vec<(String, String)> {
        parse(input.as_bytes())
            .0
            .into_iter()
            .map(|e| (e.key, e.value))
            .collect()
    }

    fn one(input: &str) -> Option<(String, String)> {
        let mut all = pairs(input);
        assert!(all.len() <= 1);
        all.pop()
    }

    #[allow(clippy::unnecessary_wraps)]
    fn pair(key: &str, value: &str) -> Option<(String, String)> {
        Some((key.to_owned(), value.to_owned()))
    }

    #[test]
    fn reads_quoted_and_unquoted_values() {
        assert_eq!(one("a = \"#ffffff\"\n"), pair("a", "#ffffff"));
        assert_eq!(one("a = '#ffffff'\n"), pair("a", "#ffffff"));
        assert_eq!(one("a = #ffffff\n"), pair("a", "#ffffff"));
        assert_eq!(one("a = \"#ffffff\" # note\n"), pair("a", "#ffffff"));
        assert_eq!(one("a = 'x\" \n"), pair("a", "x"));
        assert_eq!(one("a = \"x\n"), pair("a", "x"));
        assert_eq!(one("a = x\" y \"\n"), pair("a", " y "));
        assert_eq!(one(" a = b c \t\r\n"), pair("a", "b c"));
    }

    #[test]
    fn strips_quotes_and_spaces_from_keys() {
        assert_eq!(one("\"a\" = 1\n"), pair("a", "1"));
        assert_eq!(one("a'b c=1\n"), pair("abc", "1"));
    }

    #[test]
    fn follows_bash_read_on_extra_delimiters() {
        assert_eq!(one("a=b=\n"), pair("a", "b"));
        assert_eq!(one("a==\n"), pair("a", ""));
        assert_eq!(one("a= =\n"), pair("a", ""));
        assert_eq!(one("a = \"x=\n"), pair("a", "x"));
        assert_eq!(one("a=b==\n"), None);
        assert_eq!(one("a==b\n"), None);
    }

    #[test]
    fn keeps_lines_without_a_value() {
        assert_eq!(one("a\n"), pair("a", ""));
        assert_eq!(one("a=\n"), pair("a", ""));
    }

    #[test]
    fn skips_comments_and_empty_keys() {
        assert_eq!(pairs("# a = 1\n  #b=2\n= c\n\n"), []);
    }

    #[test]
    fn drops_an_unterminated_last_line() {
        assert_eq!(pairs("a = 1\nb = 2"), [("a".into(), "1".into())]);
    }

    #[test]
    fn drops_nul_bytes() {
        assert_eq!(one("a=x\0y\n"), pair("a", "xy"));
    }

    #[test]
    fn trims_unicode_spaces_like_glibc() {
        assert_eq!(one("a = \u{2003}b\u{3000}\n"), pair("a", "b"));
        assert_eq!(one("a = b\u{a0}\n"), None);
        assert_eq!(one("a = b\u{2007}\n"), None);
    }

    #[test]
    fn reports_skipped_lines() {
        let (entries, skipped) = parse(b"[colors]\na\t= 1\nb = x{\nc = 1\n");
        assert_eq!(entries.len(), 1);
        assert_eq!(
            skipped,
            [
                Skipped {
                    line: 1,
                    reason: SkipReason::Key
                },
                Skipped {
                    line: 2,
                    reason: SkipReason::Key
                },
                Skipped {
                    line: 3,
                    reason: SkipReason::Value { key: "b".into() }
                },
            ]
        );
    }

    #[test]
    fn rejects_values_with_unsupported_characters() {
        for value in ["x\\y", "\"x\\y\"", "[b]", "x;y", "é", "a|b", "\"x=y\""] {
            assert_eq!(one(&format!("a = {value}\n")), None, "{value}");
        }
    }
}
