//! The Omarchy sources: `colors.toml` and the legacy `alacritty.toml`.

use std::path::Path;

use super::{Resolved, SkipReason, read_alacritty, read_colors};
use crate::file::{self, ReadError};
use crate::source::{LoadError, Loaded, Source, Warning, folder_name};
use crate::theme::{Provenance, SourceKind};

/// Reads an Omarchy theme folder through its `colors.toml`.
#[derive(Debug, Clone, Copy, Default)]
pub struct OmarchyColors;

impl Source for OmarchyColors {
    fn kind(&self) -> SourceKind {
        SourceKind::OmarchyColors
    }

    fn detect(&self, dir: &Path) -> Result<bool, ReadError> {
        file::is_regular_file(&dir.join("colors.toml"))
    }

    fn load(&self, dir: &Path) -> Result<Loaded, LoadError> {
        let path = dir.join("colors.toml");
        let resolved = read_colors(&path)?;
        load(dir, &path, &resolved, self.kind())
    }
}

/// Reads an Omarchy theme folder through its legacy `alacritty.toml`, by way
/// of the `colors.toml` Omarchy generates from it.
///
/// Omarchy uses a `colors.toml` when a theme has both, so this comes after
/// [`OmarchyColors`] in [`SOURCES`](crate::source::SOURCES).
#[derive(Debug, Clone, Copy, Default)]
pub struct OmarchyAlacritty;

impl Source for OmarchyAlacritty {
    fn kind(&self) -> SourceKind {
        SourceKind::OmarchyAlacritty
    }

    fn detect(&self, dir: &Path) -> Result<bool, ReadError> {
        file::is_regular_file(&dir.join("alacritty.toml"))
    }

    fn load(&self, dir: &Path) -> Result<Loaded, LoadError> {
        let path = dir.join("alacritty.toml");
        let resolved = read_alacritty(&path)?;
        load(dir, &path, &resolved, self.kind())
    }
}

/// Builds the theme of the folder `dir` from the palette read from `path`.
fn load(
    dir: &Path,
    path: &Path,
    resolved: &Resolved,
    kind: SourceKind,
) -> Result<Loaded, LoadError> {
    let name = display_name(&folder_name(dir)?);
    let theme = resolved
        .to_theme(name, Provenance::local(kind, dir))
        .map_err(|source| LoadError::Omarchy {
            path: path.to_owned(),
            source,
        })?;
    // Lines of a generated `colors.toml` mean nothing to the reader, so
    // warnings about one carry no line number.
    let generated = kind == SourceKind::OmarchyAlacritty;
    let warnings = resolved
        .skipped()
        .iter()
        .map(|skipped| Warning {
            path: path.to_owned(),
            line: (!generated).then_some(skipped.line),
            message: match (&skipped.reason, generated) {
                (SkipReason::Key, _) => {
                    "key has unsupported characters; Omarchy ignores the line".to_owned()
                }
                (SkipReason::Value { key }, false) => {
                    format!("{key} has unsupported characters; Omarchy ignores it")
                }
                (SkipReason::Value { key }, true) => format!(
                    "{key} in the generated colors.toml has unsupported characters; \
                     Omarchy ignores it"
                ),
            },
        })
        .collect();
    Ok(Loaded { theme, warnings })
}

/// Returns the name Omarchy shows for a theme folder or repository name.
///
/// Omarchy installs `omarchy-tokyo-night-theme` as `tokyo-night` and lists it
/// as `Tokyo Night`: it drops a `.git` suffix, the `omarchy-` prefix and the
/// `-theme` suffix, lowercases the rest, capitalizes each word and turns
/// hyphens into spaces.
///
/// # Examples
///
/// ```
/// use motif_core::omarchy::display_name;
///
/// assert_eq!(display_name("omarchy-tokyo-night-theme.git"), "Tokyo Night");
/// assert_eq!(display_name("catppuccin-latte"), "Catppuccin Latte");
/// ```
#[must_use]
pub fn display_name(folder: &str) -> String {
    let name = folder.strip_suffix(".git").unwrap_or(folder);
    let name = name.strip_prefix("omarchy-").unwrap_or(name);
    let name = name.strip_suffix("-theme").unwrap_or(name);
    let mut out = String::with_capacity(name.len());
    let mut word_start = true;
    for c in name.to_lowercase().chars() {
        if c == '-' {
            out.push(' ');
            word_start = true;
        } else {
            if word_start && c.is_ascii_lowercase() {
                out.push(c.to_ascii_uppercase());
            } else {
                out.push(c);
            }
            word_start = false;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Mode;
    use std::fs;
    use std::path::{Path, PathBuf};
    use tempfile::TempDir;

    /// Creates a theme folder called `name`, removed when the `TempDir` drops.
    fn scratch(name: &str) -> (TempDir, PathBuf) {
        let tmp = TempDir::new().unwrap();
        let dir = tmp.path().join(name);
        fs::create_dir(&dir).unwrap();
        (tmp, dir)
    }

    const MINIMAL: &str = "background = \"#eeeeee\"\nforeground = \"#111111\"\n\
        color1 = \"#aa0000\"\ncolor2 = \"#00aa00\"\ncolor3 = \"#aaaa00\"\n\
        color4 = \"#0000aa\"\ncolor5 = \"#aa00aa\"\ncolor6 = \"#00aaaa\"\n";

    #[test]
    fn names_themes_like_omarchy() {
        assert_eq!(display_name("omarchy-tokyo-night-theme"), "Tokyo Night");
        assert_eq!(display_name("Omarchy-Retro-82"), "Omarchy Retro 82");
        assert_eq!(display_name("rose-pine"), "Rose Pine");
        assert_eq!(display_name("a--b"), "A  B");
        assert_eq!(display_name(""), "");
    }

    #[test]
    fn detects_and_loads_a_folder() {
        let (_tmp, dir) = scratch("omarchy-paper-theme");
        assert!(!OmarchyColors.detect(&dir).unwrap());
        fs::write(dir.join("colors.toml"), format!("{MINIMAL}[colors]\n")).unwrap();
        assert!(OmarchyColors.detect(&dir).unwrap());

        let loaded = crate::source::load(&dir).unwrap();
        assert_eq!(loaded.theme.name, "Paper");
        assert_eq!(loaded.theme.mode, Mode::Light);
        assert_eq!(loaded.theme.provenance.path.as_deref(), Some(dir.as_path()));
        assert_eq!(loaded.warnings.len(), 1);
        assert_eq!(loaded.warnings[0].line, Some(9));
    }

    #[test]
    fn honors_a_light_mode_file() {
        let (_tmp, dir) = scratch("marker");
        fs::write(
            dir.join("colors.toml"),
            MINIMAL.replace("#eeeeee", "#111111"),
        )
        .unwrap();
        assert_eq!(OmarchyColors.load(&dir).unwrap().theme.mode, Mode::Dark);
        fs::write(dir.join("light.mode"), "").unwrap();
        assert_eq!(OmarchyColors.load(&dir).unwrap().theme.mode, Mode::Light);
    }

    #[test]
    fn refuses_symlinks() {
        let (_tmp, dir) = scratch("links");
        fs::write(dir.join("real.toml"), MINIMAL).unwrap();
        std::os::unix::fs::symlink("real.toml", dir.join("colors.toml")).unwrap();
        assert!(matches!(
            OmarchyColors.detect(&dir),
            Err(ReadError::Symlink { .. })
        ));
        assert!(matches!(
            OmarchyColors.load(&dir),
            Err(LoadError::Read(ReadError::Symlink { .. }))
        ));

        fs::remove_file(dir.join("colors.toml")).unwrap();
        fs::rename(dir.join("real.toml"), dir.join("colors.toml")).unwrap();
        std::os::unix::fs::symlink("colors.toml", dir.join("light.mode")).unwrap();
        assert!(matches!(
            OmarchyColors.load(&dir),
            Err(LoadError::Read(ReadError::Symlink { .. }))
        ));
    }

    #[test]
    fn reports_an_incomplete_theme() {
        let (_tmp, dir) = scratch("incomplete");
        fs::write(dir.join("colors.toml"), "mode = \"dark\"\n").unwrap();
        let err = OmarchyColors.load(&dir).unwrap_err();
        assert!(matches!(err, LoadError::Omarchy { .. }));
        assert!(
            err.to_string()
                .ends_with("does not describe a complete theme")
        );
    }

    #[test]
    fn reports_an_unrecognized_folder() {
        let (_tmp, dir) = scratch("empty");
        assert!(matches!(
            crate::source::load(&dir),
            Err(LoadError::Unrecognized { .. })
        ));
    }

    #[test]
    fn names_a_theme_read_through_a_path_without_a_name() {
        let (_tmp, dir) = scratch("omarchy-paper-theme");
        fs::write(dir.join("colors.toml"), MINIMAL).unwrap();
        fs::create_dir(dir.join("sub")).unwrap();
        let unnamed = dir.join("sub").join("..");
        assert_eq!(unnamed.file_name(), None);
        assert_eq!(OmarchyColors.load(&unnamed).unwrap().theme.name, "Paper");
    }

    #[test]
    fn names_the_current_folder() {
        // Tests share the process's working directory, so this only reads it.
        let cwd = std::env::current_dir().unwrap();
        let expected = cwd.file_name().unwrap().to_string_lossy().into_owned();
        assert_eq!(folder_name(Path::new(".")).unwrap(), expected);
    }

    const ALACRITTY: &str = "[colors.primary]\nbackground = '#eeeeee'\n\
        foreground = '#111111'\n[colors.normal]\nblack = '#000000'\n\
        red = '#aa0000'\ngreen = '#00aa00'\nyellow = '#aaaa00'\n\
        blue = '#0000aa'\nmagenta = '#aa00aa'\ncyan = '#00aaaa'\n\
        white = '#aaaaaa'\n";

    #[test]
    fn detects_and_loads_an_alacritty_folder() {
        let (_tmp, dir) = scratch("omarchy-old-paper-theme");
        assert!(!OmarchyAlacritty.detect(&dir).unwrap());
        fs::write(dir.join("alacritty.toml"), ALACRITTY).unwrap();
        assert!(OmarchyAlacritty.detect(&dir).unwrap());
        assert!(!OmarchyColors.detect(&dir).unwrap());

        let loaded = crate::source::load(&dir).unwrap();
        assert_eq!(loaded.theme.name, "Old Paper");
        assert_eq!(loaded.theme.mode, Mode::Light);
        assert_eq!(loaded.theme.provenance.kind, SourceKind::OmarchyAlacritty);
        assert_eq!(loaded.warnings, []);
    }

    #[test]
    fn prefers_colors_toml_to_alacritty_toml() {
        let (_tmp, dir) = scratch("both");
        fs::write(dir.join("alacritty.toml"), ALACRITTY).unwrap();
        fs::write(
            dir.join("colors.toml"),
            MINIMAL.replace("#eeeeee", "#222222"),
        )
        .unwrap();
        let loaded = crate::source::load(&dir).unwrap();
        assert_eq!(loaded.theme.provenance.kind, SourceKind::OmarchyColors);
        assert_eq!(loaded.theme.mode, Mode::Dark);
    }

    #[test]
    fn honors_a_light_mode_file_beside_alacritty_toml() {
        let (_tmp, dir) = scratch("marker");
        fs::write(
            dir.join("alacritty.toml"),
            ALACRITTY.replace("#eeeeee", "#222222"),
        )
        .unwrap();
        assert_eq!(OmarchyAlacritty.load(&dir).unwrap().theme.mode, Mode::Dark);
        fs::write(dir.join("light.mode"), "").unwrap();
        assert_eq!(OmarchyAlacritty.load(&dir).unwrap().theme.mode, Mode::Light);
    }

    #[test]
    fn reports_missing_alacritty_colors() {
        let (_tmp, dir) = scratch("partial");
        fs::write(
            dir.join("alacritty.toml"),
            ALACRITTY.replace("cyan", "#cyan"),
        )
        .unwrap();
        let err = OmarchyAlacritty.load(&dir).unwrap_err();
        assert!(matches!(err, LoadError::OmarchyAlacritty { .. }));
        assert!(
            err.to_string()
                .starts_with("cannot extract all normal colors from ")
        );
        let cause = std::error::Error::source(&err).unwrap().to_string();
        assert_eq!(cause, "missing colors.normal.cyan");
    }

    #[test]
    fn warns_about_a_generated_value_omarchy_ignores() {
        let (_tmp, dir) = scratch("tabs");
        let text = format!("{ALACRITTY}[colors.bright]\nred\tx = '#ff0000'\n");
        fs::write(dir.join("alacritty.toml"), text).unwrap();
        let loaded = OmarchyAlacritty.load(&dir).unwrap();
        assert_eq!(loaded.warnings.len(), 1);
        assert_eq!(loaded.warnings[0].line, None);
        assert!(
            loaded.warnings[0]
                .message
                .starts_with("color9 in the generated")
        );
    }

    #[test]
    fn refuses_unsafe_alacritty_files() {
        let (_tmp, dir) = scratch("unsafe");
        fs::write(dir.join("real.toml"), ALACRITTY).unwrap();
        std::os::unix::fs::symlink("real.toml", dir.join("alacritty.toml")).unwrap();
        assert!(matches!(
            OmarchyAlacritty.detect(&dir),
            Err(ReadError::Symlink { .. })
        ));
        assert!(matches!(
            crate::source::load(&dir),
            Err(LoadError::Read(ReadError::Symlink { .. }))
        ));

        fs::remove_file(dir.join("alacritty.toml")).unwrap();
        let mut large = ALACRITTY.to_owned();
        while large.len() as u64 <= super::super::MAX_ALACRITTY_SIZE {
            large.push_str("# padding\n");
        }
        fs::write(dir.join("alacritty.toml"), large).unwrap();
        assert!(matches!(
            OmarchyAlacritty.load(&dir),
            Err(LoadError::Read(ReadError::TooLarge { .. }))
        ));
    }

    #[test]
    fn reports_a_folder_with_neither_color_file() {
        let (_tmp, dir) = scratch("omarchy-gruvu-theme");
        fs::write(dir.join("hyprland.conf"), "general {}\n").unwrap();
        fs::create_dir(dir.join("backgrounds")).unwrap();
        let err = crate::source::load(&dir).unwrap_err();
        assert!(matches!(err, LoadError::Unrecognized { .. }));
        assert!(
            err.to_string()
                .ends_with("has no theme file motif can read")
        );
    }

    #[test]
    fn refuses_to_name_the_root() {
        assert!(matches!(
            folder_name(Path::new("/")),
            Err(LoadError::Unnamed { .. })
        ));
    }
}
