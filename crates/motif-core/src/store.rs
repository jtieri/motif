//! Writing motif's output files.

use std::fs::{self, File, Permissions};
use std::io::{self, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::{Component, Path, PathBuf};

/// Whether a write may replace an existing file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Overwrite {
    /// Fail if the file exists.
    Never,
    /// Replace the file if it exists.
    Allow,
}

/// The error returned when a file cannot be written.
#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum WriteError {
    /// The file exists and overwriting was not allowed.
    #[error("{} already exists", path.display())]
    Exists {
        /// The file.
        path: PathBuf,
    },
    /// The folder to write into is missing or not a folder.
    #[error("{} is not a folder", path.display())]
    NotAFolder {
        /// The folder.
        path: PathBuf,
    },
    /// The file name is not a single plain path component.
    #[error("{name:?} is not a plain file name")]
    BadName {
        /// The name.
        name: String,
    },
    /// Writing, syncing or renaming failed.
    #[error("cannot write {}", path.display())]
    Io {
        /// The file or folder being written.
        path: PathBuf,
        /// The underlying error.
        #[source]
        source: io::Error,
    },
}

/// Writes `contents` to the file `name` in the folder `dir`, atomically, and
/// returns the file's path.
///
/// The contents go to a temporary file in `dir`, which is synced to disk and
/// then renamed over `name`, so the file is either absent, its old version
/// or the new one, never partly written. With [`Overwrite::Never`] the rename
/// refuses an existing file, even one created after this call began. The
/// folder is synced last so that the rename survives a crash. The new file
/// is readable by everyone and writable by its owner, like the dotfiles'
/// other configuration files.
///
/// # Errors
///
/// Returns [`WriteError::BadName`] if `name` is not a plain file name,
/// [`WriteError::NotAFolder`] if `dir` is not an existing folder,
/// [`WriteError::Exists`] if the file exists and `overwrite` is
/// [`Overwrite::Never`], and [`WriteError::Io`] if a step fails. The
/// temporary file is removed on failure.
pub fn write_atomic(
    dir: &Path,
    name: &str,
    contents: &[u8],
    overwrite: Overwrite,
) -> Result<PathBuf, WriteError> {
    let mut components = Path::new(name).components();
    if !matches!(
        (components.next(), components.next()),
        (Some(Component::Normal(_)), None)
    ) {
        return Err(WriteError::BadName {
            name: name.to_owned(),
        });
    }
    if !fs::metadata(dir).is_ok_and(|m| m.is_dir()) {
        return Err(WriteError::NotAFolder {
            path: dir.to_owned(),
        });
    }

    let path = dir.join(name);
    let io_error = |path: &Path| {
        let path = path.to_owned();
        move |source| WriteError::Io { path, source }
    };
    let mut temp = tempfile::Builder::new()
        .prefix(".motif-")
        .suffix(".tmp")
        .tempfile_in(dir)
        .map_err(io_error(dir))?;
    temp.write_all(contents).map_err(io_error(temp.path()))?;
    temp.as_file()
        .set_permissions(Permissions::from_mode(0o644))
        .map_err(io_error(temp.path()))?;
    temp.as_file().sync_all().map_err(io_error(temp.path()))?;

    let persisted = match overwrite {
        Overwrite::Never => temp.persist_noclobber(&path),
        Overwrite::Allow => temp.persist(&path),
    };
    if let Err(err) = persisted {
        // Dropping the returned temporary file removes it.
        return Err(if err.error.kind() == io::ErrorKind::AlreadyExists {
            WriteError::Exists { path }
        } else {
            io_error(&path)(err.error)
        });
    }

    File::open(dir)
        .and_then(|folder| folder.sync_all())
        .map_err(io_error(dir))?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn entries(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(dir)
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    #[test]
    fn writes_a_new_file() {
        let tmp = TempDir::new().unwrap();
        let path = write_atomic(tmp.path(), "a.json", b"{}\n", Overwrite::Never).unwrap();
        assert_eq!(path, tmp.path().join("a.json"));
        assert_eq!(fs::read(&path).unwrap(), b"{}\n");
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o644
        );
        assert_eq!(entries(tmp.path()), ["a.json"]);
    }

    #[test]
    fn refuses_to_overwrite_unless_allowed() {
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("a.json"), b"old").unwrap();
        let err = write_atomic(tmp.path(), "a.json", b"new", Overwrite::Never).unwrap_err();
        assert!(matches!(err, WriteError::Exists { .. }));
        assert_eq!(fs::read(tmp.path().join("a.json")).unwrap(), b"old");
        assert_eq!(entries(tmp.path()), ["a.json"]);

        write_atomic(tmp.path(), "a.json", b"new", Overwrite::Allow).unwrap();
        assert_eq!(fs::read(tmp.path().join("a.json")).unwrap(), b"new");
    }

    #[test]
    fn replaces_a_symlink_rather_than_its_target() {
        let tmp = TempDir::new().unwrap();
        let target = tmp.path().join("target");
        fs::write(&target, b"keep").unwrap();
        std::os::unix::fs::symlink(&target, tmp.path().join("a.json")).unwrap();
        assert!(matches!(
            write_atomic(tmp.path(), "a.json", b"new", Overwrite::Never),
            Err(WriteError::Exists { .. })
        ));
        write_atomic(tmp.path(), "a.json", b"new", Overwrite::Allow).unwrap();
        assert_eq!(fs::read(&target).unwrap(), b"keep");
        assert!(
            !fs::symlink_metadata(tmp.path().join("a.json"))
                .unwrap()
                .is_symlink()
        );
    }

    #[test]
    fn refuses_names_that_leave_the_folder() {
        let tmp = TempDir::new().unwrap();
        for name in ["", ".", "..", "../a.json", "sub/a.json", "/etc/a.json"] {
            assert!(
                matches!(
                    write_atomic(tmp.path(), name, b"", Overwrite::Allow),
                    Err(WriteError::BadName { .. })
                ),
                "{name:?}"
            );
        }
        assert_eq!(entries(tmp.path()), Vec::<String>::new());
    }

    #[test]
    fn refuses_a_missing_folder() {
        let tmp = TempDir::new().unwrap();
        let missing = tmp.path().join("missing");
        assert!(matches!(
            write_atomic(&missing, "a.json", b"", Overwrite::Allow),
            Err(WriteError::NotAFolder { .. })
        ));
        assert!(!missing.exists());
    }
}
