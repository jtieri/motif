//! Reading theme files without following symbolic links.

use std::fs::{self, OpenOptions};
use std::io::{self, Read};
use std::os::unix::fs::OpenOptionsExt;
use std::path::{Path, PathBuf};

/// The error returned when a theme file cannot be read.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum ReadError {
    /// The path is a symbolic link, which motif never follows.
    #[error("{} is a symbolic link", path.display())]
    Symlink {
        /// The offending path.
        path: PathBuf,
    },
    /// The path is not a regular file.
    #[error("{} is not a regular file", path.display())]
    NotAFile {
        /// The offending path.
        path: PathBuf,
    },
    /// The file is larger than the caller allows.
    #[error("{} is larger than {limit} bytes", path.display())]
    TooLarge {
        /// The offending path.
        path: PathBuf,
        /// The largest size allowed, in bytes.
        limit: u64,
    },
    /// Opening or reading the file failed.
    #[error("cannot read {}", path.display())]
    Io {
        /// The path being read.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: io::Error,
    },
}

/// Reads a whole regular file of at most `limit` bytes.
///
/// The final component of `path` must not be a symbolic link. Special files
/// such as FIFOs and devices are refused without blocking on them.
///
/// # Errors
///
/// Returns [`ReadError::Symlink`] for a symbolic link, [`ReadError::NotAFile`]
/// for anything other than a regular file, [`ReadError::TooLarge`] if the file
/// holds more than `limit` bytes and [`ReadError::Io`] if it cannot be opened
/// or read.
pub fn read_capped(path: &Path, limit: u64) -> Result<Vec<u8>, ReadError> {
    let io_error = |source| ReadError::Io {
        path: path.to_owned(),
        source,
    };

    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK)
        .open(path)
        .map_err(|source| {
            if source.raw_os_error() == Some(libc::ELOOP) {
                ReadError::Symlink {
                    path: path.to_owned(),
                }
            } else {
                io_error(source)
            }
        })?;

    let metadata = file.metadata().map_err(io_error)?;
    if !metadata.is_file() {
        return Err(ReadError::NotAFile {
            path: path.to_owned(),
        });
    }
    let too_large = || ReadError::TooLarge {
        path: path.to_owned(),
        limit,
    };
    if metadata.len() > limit {
        return Err(too_large());
    }

    // The file may grow after the size check, so the read is capped as well.
    let mut bytes = Vec::new();
    Read::take(&file, limit.saturating_add(1))
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    if bytes.len() as u64 > limit {
        return Err(too_large());
    }
    Ok(bytes)
}

/// Reports whether `path` is a regular file, without following a symbolic link.
///
/// # Errors
///
/// Returns [`ReadError::Symlink`] if `path` is a symbolic link and
/// [`ReadError::Io`] if its metadata cannot be read for a reason other than
/// the path not existing.
pub fn is_regular_file(path: &Path) -> Result<bool, ReadError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.is_symlink() => Err(ReadError::Symlink {
            path: path.to_owned(),
        }),
        Ok(metadata) => Ok(metadata.is_file()),
        Err(err) if err.kind() == io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(ReadError::Io {
            path: path.to_owned(),
            source,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::os::unix::fs::symlink;

    use tempfile::TempDir;

    #[test]
    fn reads_a_small_file() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();
        let path = dir.join("colors.toml");
        fs::write(&path, b"a = 1\n").unwrap();
        assert_eq!(read_capped(&path, 6).unwrap(), b"a = 1\n");
        assert!(is_regular_file(&path).unwrap());
    }

    #[test]
    fn refuses_a_file_over_the_limit() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();
        let path = dir.join("colors.toml");
        fs::write(&path, b"1234567").unwrap();
        assert!(matches!(
            read_capped(&path, 6),
            Err(ReadError::TooLarge { limit: 6, .. })
        ));
    }

    #[test]
    fn refuses_symlinks() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();
        let target = dir.join("target");
        let link = dir.join("colors.toml");
        fs::write(&target, b"a = 1\n").unwrap();
        symlink(&target, &link).unwrap();
        assert!(matches!(
            read_capped(&link, 64),
            Err(ReadError::Symlink { .. })
        ));
        assert!(matches!(
            is_regular_file(&link),
            Err(ReadError::Symlink { .. })
        ));
    }

    #[test]
    fn refuses_directories_and_reports_missing_files() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();
        assert!(matches!(
            read_capped(dir, 64),
            Err(ReadError::NotAFile { .. })
        ));
        assert!(!is_regular_file(dir).unwrap());
        assert!(!is_regular_file(&dir.join("missing")).unwrap());
        assert!(matches!(
            read_capped(&dir.join("missing"), 64),
            Err(ReadError::Io { .. })
        ));
    }

    #[test]
    fn refuses_a_fifo_without_blocking() {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path();
        let path = dir.join("colors.toml");
        let made = std::process::Command::new("mkfifo")
            .arg(&path)
            .status()
            .unwrap();
        assert!(made.success());
        assert!(matches!(
            read_capped(&path, 64),
            Err(ReadError::NotAFile { .. })
        ));
    }
}
