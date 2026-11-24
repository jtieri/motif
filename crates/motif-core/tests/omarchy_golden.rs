//! Checks the Omarchy port against `omarchy-theme-color --all`.
//!
//! The expected files are the resolver's own output, written by
//! `scripts/omarchy-golden.sh`. `themes/` holds Omarchy's built-in themes and
//! `cases/` holds inputs written to exercise the parser and the cascade.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use motif_core::omarchy::{OmarchyColors, read_colors};
use motif_core::{Source, SourceKind};

fn fixtures(group: &str) -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/omarchy")
        .join(group);
    let mut dirs: Vec<PathBuf> = fs::read_dir(root)
        .expect("fixture folder")
        .map(|entry| entry.expect("fixture entry").path())
        .filter(|path| path.join("colors.toml").is_file())
        .collect();
    dirs.sort();
    dirs
}

fn check(dir: &Path) {
    let expected = fs::read_to_string(dir.join("expected.tsv")).expect("expected.tsv");
    let resolved = read_colors(&dir.join("colors.toml")).expect("colors.toml");
    let actual = resolved
        .iter()
        .fold(String::new(), |mut out, (key, value)| {
            let _ = writeln!(out, "{key}\t{value}");
            out
        });
    assert_eq!(actual, expected, "{}", dir.display());
}

#[test]
fn built_in_themes_resolve_like_omarchy() {
    let themes = fixtures("themes");
    assert_eq!(themes.len(), 23);
    for dir in &themes {
        check(dir);
    }
}

#[test]
fn edge_cases_resolve_like_omarchy() {
    let cases = fixtures("cases");
    assert!(!cases.is_empty());
    for dir in &cases {
        check(dir);
    }
}

#[test]
fn built_in_themes_load_cleanly() {
    for dir in fixtures("themes") {
        assert!(OmarchyColors.detect(&dir).unwrap());
        let loaded = OmarchyColors.load(&dir).unwrap();
        assert!(loaded.warnings.is_empty(), "{}", dir.display());

        let theme = loaded.theme;
        assert!(!theme.name.is_empty());
        assert_eq!(theme.provenance.kind, SourceKind::OmarchyColors);
        assert_eq!(
            theme.mode.as_str(),
            read_colors(&dir.join("colors.toml"))
                .unwrap()
                .get("mode")
                .unwrap()
        );
        assert!(theme.hues.len() >= 6, "{}", dir.display());
    }
}
