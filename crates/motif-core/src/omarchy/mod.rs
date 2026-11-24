//! Omarchy themes.
//!
//! An Omarchy theme is a folder whose `colors.toml` names its palette. Omarchy
//! reads that file with `omarchy-theme-color`, which fills in every key the
//! theme leaves out through a cascade of aliases and fallbacks. This module
//! ports that script exactly: [`ColorsToml::resolve`] returns the same keys
//! and values `omarchy-theme-color --all` prints, and [`Resolved::to_theme`]
//! turns them into a [`Theme`](crate::Theme).
//!
//! Older themes have only an `alacritty.toml`, from which Omarchy generates
//! the `colors.toml` with `omarchy-theme-colors-from-alacritty`.
//! [`colors_from_alacritty`] ports that script, and [`read_alacritty`] reads
//! such a theme through the same resolver.

mod alacritty;
mod convert;
mod parse;
mod resolve;
mod source;
mod value;

use std::collections::BTreeMap;
use std::path::Path;

pub use alacritty::{MissingColors, colors_from_alacritty};
pub use convert::{ThemeError, ThemeErrorKind};
pub use parse::{Entry, SkipReason, Skipped};
pub use source::{OmarchyAlacritty, OmarchyColors, display_name};
pub use value::{ParseValueError, parse_color};

use crate::file::{self, ReadError};
use crate::source::LoadError;

/// The largest `colors.toml` motif reads, in bytes.
pub const MAX_COLORS_SIZE: u64 = 64 * 1024;

/// The largest `alacritty.toml` motif reads, in bytes.
pub const MAX_ALACRITTY_SIZE: u64 = 64 * 1024;

/// The pairs Omarchy reads from a `colors.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColorsToml {
    entries: Vec<Entry>,
    skipped: Vec<Skipped>,
}

impl ColorsToml {
    /// Reads the contents of a `colors.toml` the way Omarchy does.
    ///
    /// This never fails: Omarchy skips lines it cannot use, and so does this.
    /// The skipped lines are available from [`skipped`](Self::skipped).
    #[must_use]
    pub fn parse(bytes: &[u8]) -> Self {
        let (entries, skipped) = parse::parse(bytes);
        Self { entries, skipped }
    }

    /// Returns the accepted pairs in file order, duplicates included.
    #[must_use]
    pub fn entries(&self) -> &[Entry] {
        &self.entries
    }

    /// Returns the lines Omarchy skips because of unsupported characters.
    #[must_use]
    pub fn skipped(&self) -> &[Skipped] {
        &self.skipped
    }

    /// Resolves the full palette.
    ///
    /// `light_mode_file` says whether a `light.mode` file sits beside the
    /// `colors.toml`, which makes a theme without a `mode` key light.
    #[must_use]
    pub fn resolve(&self, light_mode_file: bool) -> Resolved {
        let mut file_keys: Vec<String> = Vec::new();
        for entry in &self.entries {
            if !file_keys.contains(&entry.key) {
                file_keys.push(entry.key.clone());
            }
        }
        Resolved {
            values: resolve::resolve(&self.entries, light_mode_file),
            file_keys,
            skipped: self.skipped.clone(),
        }
    }
}

/// A theme's full palette as Omarchy resolves it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Resolved {
    values: BTreeMap<String, String>,
    file_keys: Vec<String>,
    skipped: Vec<Skipped>,
}

impl Resolved {
    /// Returns a key's value. Omarchy leaves some keys present but empty.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&str> {
        self.values.get(key).map(String::as_str)
    }

    /// Returns every key and value in byte order of the keys, as
    /// `omarchy-theme-color --all` prints them.
    pub fn iter(&self) -> impl Iterator<Item = (&str, &str)> {
        self.values.iter().map(|(k, v)| (k.as_str(), v.as_str()))
    }

    /// Returns the keys the file itself sets, in the order they first appear.
    #[must_use]
    pub fn file_keys(&self) -> &[String] {
        &self.file_keys
    }

    /// Returns the lines Omarchy skipped while reading the file.
    #[must_use]
    pub fn skipped(&self) -> &[Skipped] {
        &self.skipped
    }
}

/// Reads and resolves a `colors.toml`, checking for a `light.mode` file beside
/// it as Omarchy does.
///
/// # Errors
///
/// Returns a [`ReadError`] if the file, or a `light.mode` beside it, is a
/// symbolic link, if the file is not a regular file, is larger than
/// [`MAX_COLORS_SIZE`] or cannot be read.
pub fn read_colors(path: &Path) -> Result<Resolved, ReadError> {
    let bytes = file::read_capped(path, MAX_COLORS_SIZE)?;
    Ok(ColorsToml::parse(&bytes).resolve(light_mode_beside(path)?))
}

/// Reads an `alacritty.toml`, generates the `colors.toml` Omarchy would and
/// resolves it, checking for a `light.mode` file beside it as Omarchy does.
///
/// # Errors
///
/// Returns [`LoadError::Read`] for the same reasons as [`read_colors`], with
/// [`MAX_ALACRITTY_SIZE`] as the limit, and
/// [`LoadError::OmarchyAlacritty`] if the file lacks a normal color.
pub fn read_alacritty(path: &Path) -> Result<Resolved, LoadError> {
    let bytes = file::read_capped(path, MAX_ALACRITTY_SIZE)?;
    let colors = colors_from_alacritty(&bytes).map_err(|source| LoadError::OmarchyAlacritty {
        path: path.to_owned(),
        source,
    })?;
    Ok(ColorsToml::parse(&colors).resolve(light_mode_beside(path)?))
}

/// Reports whether a `light.mode` file sits beside `path`.
fn light_mode_beside(path: &Path) -> Result<bool, ReadError> {
    match path.parent() {
        Some(dir) => file::is_regular_file(&dir.join("light.mode")),
        None => Ok(false),
    }
}
