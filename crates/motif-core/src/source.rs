//! Theme formats and how to tell them apart.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use crate::file::ReadError;
use crate::omarchy::{MissingColors, OmarchyAlacritty, OmarchyColors, ThemeError};
use crate::theme::{SourceKind, Theme};

/// A theme format: recognizes a theme folder in that format and reads it.
pub trait Source: fmt::Debug + Send + Sync {
    /// Returns the format this source reads.
    fn kind(&self) -> SourceKind;

    /// Reports whether `dir` holds a theme in this format.
    ///
    /// Detection looks only at which files exist and never reads them.
    ///
    /// # Errors
    ///
    /// Returns a [`ReadError`] if a file the format needs is a symbolic link
    /// or its metadata cannot be read.
    fn detect(&self, dir: &Path) -> Result<bool, ReadError>;

    /// Reads the theme in `dir`.
    ///
    /// # Errors
    ///
    /// Returns a [`LoadError`] if the files cannot be read or do not describe
    /// a complete theme.
    fn load(&self, dir: &Path) -> Result<Loaded, LoadError>;
}

/// The formats motif reads, in the order they are tried.
///
/// A folder in more than one format is read with the first that matches, so
/// a format that Omarchy prefers comes before the ones it falls back to.
pub static SOURCES: &[&dyn Source] = &[&OmarchyColors, &OmarchyAlacritty];

/// Returns the first source that recognizes `dir`.
///
/// # Errors
///
/// Returns a [`ReadError`] if a source cannot inspect `dir`.
pub fn detect(dir: &Path) -> Result<Option<&'static dyn Source>, ReadError> {
    for &source in SOURCES {
        if source.detect(dir)? {
            return Ok(Some(source));
        }
    }
    Ok(None)
}

/// Detects the format of the theme in `dir` and reads it.
///
/// # Errors
///
/// Returns [`LoadError::Unrecognized`] if no source recognizes `dir`, and
/// otherwise any error from detecting or loading.
pub fn load(dir: &Path) -> Result<Loaded, LoadError> {
    match detect(dir)? {
        Some(source) => source.load(dir),
        None => Err(LoadError::Unrecognized {
            path: dir.to_owned(),
        }),
    }
}

/// A theme read by a source, with anything the source skipped along the way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Loaded {
    /// The theme.
    pub theme: Theme,
    /// Problems that did not stop the theme from loading.
    pub warnings: Vec<Warning>,
}

/// A problem in a theme file that did not stop it from loading.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Warning {
    /// The file with the problem.
    pub path: PathBuf,
    /// The 1-based line of the problem, if it has one.
    pub line: Option<usize>,
    /// What the problem is.
    pub message: String,
}

impl fmt::Display for Warning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.path.display())?;
        if let Some(line) = self.line {
            write!(f, ":{line}")?;
        }
        write!(f, ": {}", self.message)
    }
}

/// The error returned when a theme cannot be loaded.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum LoadError {
    /// A file could not be read safely.
    #[error(transparent)]
    Read(#[from] ReadError),
    /// No source recognizes the folder.
    #[error("{} has no theme file motif can read", path.display())]
    Unrecognized {
        /// The folder.
        path: PathBuf,
    },
    /// An Omarchy palette is missing a color or has one motif cannot read.
    #[error("{} does not describe a complete theme", path.display())]
    Omarchy {
        /// The `colors.toml`, or the `alacritty.toml` it was generated from.
        path: PathBuf,
        /// What is wrong with the palette.
        #[source]
        source: ThemeError,
    },
    /// An Omarchy `alacritty.toml` lacks a color Omarchy needs to generate a
    /// `colors.toml` from it.
    #[error("cannot extract all normal colors from {}", path.display())]
    OmarchyAlacritty {
        /// The `alacritty.toml`.
        path: PathBuf,
        /// The colors it lacks.
        #[source]
        source: MissingColors,
    },
    /// The folder has no name to name the theme by, as with `/`.
    #[error("{} has no folder name to name the theme by", path.display())]
    Unnamed {
        /// The folder.
        path: PathBuf,
    },
}

/// Returns the last component of `dir`, resolving paths such as `.` or `..`
/// that do not end in a name.
pub(crate) fn folder_name(dir: &Path) -> Result<String, LoadError> {
    if let Some(name) = dir.file_name() {
        return Ok(name.to_string_lossy().into_owned());
    }
    let canonical = fs::canonicalize(dir).map_err(|source| ReadError::Io {
        path: dir.to_owned(),
        source,
    })?;
    canonical
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .ok_or_else(|| LoadError::Unnamed {
            path: dir.to_owned(),
        })
}
