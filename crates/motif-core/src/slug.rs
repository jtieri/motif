//! Palette slugs: the file stem a palette is saved and applied under.

use std::fmt;
use std::str::FromStr;

/// The longest slug allowed, in bytes.
pub const MAX_LEN: usize = 64;

/// The slug the dotfiles reserve for the palette derived from the current
/// wallpaper, which is rewritten on every wallpaper change.
pub const RESERVED: &str = "wallpaper-derived";

/// A palette's slug, such as `catppuccin-mocha`.
///
/// A slug is 1 to 64 lowercase ASCII letters, digits and hyphens, starting
/// with a letter or digit, and is not `wallpaper-derived`. It names a file in
/// the palette folder and is passed to shell scripts, so nothing else is
/// allowed.
///
/// # Examples
///
/// ```
/// use motif_core::Slug;
///
/// let slug: Slug = "tokyo-night".parse()?;
/// assert_eq!(slug.as_str(), "tokyo-night");
/// assert_eq!(Slug::from_name("Rosé Pine Moon")?.as_str(), "ros-pine-moon");
/// assert!("Tokyo Night".parse::<Slug>().is_err());
/// # Ok::<(), motif_core::SlugError>(())
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Slug(String);

impl Slug {
    /// Makes a slug from a display name: ASCII letters are lowercased, every
    /// run of other characters becomes one hyphen, and hyphens are trimmed
    /// from both ends.
    ///
    /// # Errors
    ///
    /// Returns a [`SlugError`] if the result is empty, too long or reserved.
    pub fn from_name(name: &str) -> Result<Self, SlugError> {
        let mut slug = String::with_capacity(name.len());
        for c in name.chars() {
            if c.is_ascii_alphanumeric() {
                slug.push(c.to_ascii_lowercase());
            } else if !slug.is_empty() && !slug.ends_with('-') {
                slug.push('-');
            }
        }
        let trimmed = slug.trim_end_matches('-');
        trimmed.parse()
    }

    /// Returns the slug as a string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl FromStr for Slug {
    type Err = SlugError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let error = |kind| SlugError {
            slug: s.to_owned(),
            kind,
        };
        if s.is_empty() {
            return Err(error(SlugErrorKind::Empty));
        }
        if s.len() > MAX_LEN {
            return Err(error(SlugErrorKind::TooLong));
        }
        if !s
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return Err(error(SlugErrorKind::Charset));
        }
        if s.starts_with('-') {
            return Err(error(SlugErrorKind::LeadingHyphen));
        }
        if s == RESERVED {
            return Err(error(SlugErrorKind::Reserved));
        }
        Ok(Self(s.to_owned()))
    }
}

impl fmt::Display for Slug {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl serde::Serialize for Slug {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(&self.0)
    }
}

impl AsRef<str> for Slug {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// The error returned when a string is not a valid slug.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("invalid slug {slug:?}: {kind}")]
pub struct SlugError {
    slug: String,
    kind: SlugErrorKind,
}

impl SlugError {
    /// Returns what is wrong with the slug.
    #[must_use]
    pub fn kind(&self) -> SlugErrorKind {
        self.kind
    }
}

/// What is wrong with a slug.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum SlugErrorKind {
    /// The slug is empty, or a name has no ASCII letters or digits to make
    /// one from.
    #[error("it is empty")]
    Empty,
    /// The slug is longer than [`MAX_LEN`] bytes.
    #[error("it is longer than {MAX_LEN} characters")]
    TooLong,
    /// The slug has a character other than `a-z`, `0-9` and `-`.
    #[error("only a-z, 0-9 and - are allowed")]
    Charset,
    /// The slug starts with a hyphen.
    #[error("it must start with a letter or digit")]
    LeadingHyphen,
    /// The slug is [`RESERVED`].
    #[error("it is reserved for the wallpaper-derived palette")]
    Reserved,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn makes_slugs_from_names() {
        let slug = |name| Slug::from_name(name).map(|s| s.0);
        assert_eq!(slug("Catppuccin Mocha").unwrap(), "catppuccin-mocha");
        assert_eq!(slug("  Retro 82!! ").unwrap(), "retro-82");
        assert_eq!(slug("--a__b--").unwrap(), "a-b");
        assert_eq!(
            slug("Wallpaper Derived").unwrap_err().kind(),
            SlugErrorKind::Reserved
        );
        assert_eq!(slug("ñ").unwrap_err().kind(), SlugErrorKind::Empty);
        assert_eq!(
            slug(&"a".repeat(65)).unwrap_err().kind(),
            SlugErrorKind::TooLong
        );
    }

    #[test]
    fn checks_slugs() {
        for ok in ["a", "0", "tokyo-night", "a-", &"z".repeat(64)] {
            assert!(ok.parse::<Slug>().is_ok(), "{ok:?}");
        }
        for (bad, kind) in [
            ("", SlugErrorKind::Empty),
            ("Tokyo", SlugErrorKind::Charset),
            ("a b", SlugErrorKind::Charset),
            ("../x", SlugErrorKind::Charset),
            ("a_b", SlugErrorKind::Charset),
            ("é", SlugErrorKind::Charset),
            ("-rf", SlugErrorKind::LeadingHyphen),
            ("wallpaper-derived", SlugErrorKind::Reserved),
        ] {
            assert_eq!(bad.parse::<Slug>().unwrap_err().kind(), kind, "{bad:?}");
        }
        assert_eq!(
            "Tokyo".parse::<Slug>().unwrap_err().to_string(),
            "invalid slug \"Tokyo\": only a-z, 0-9 and - are allowed"
        );
    }
}
