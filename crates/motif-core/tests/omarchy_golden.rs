//! Checks the Omarchy port against `omarchy-theme-color --all`.
//!
//! The expected files are the resolver's own output, written by
//! `scripts/omarchy-golden.sh`. `themes/` holds Omarchy's built-in themes and
//! `cases/` holds inputs written to exercise the parser and the cascade.
//!
//! `alacritty/` holds legacy `alacritty.toml` inputs. Beside each is the
//! `colors.toml` that `omarchy-theme-colors-from-alacritty` generates from it,
//! as `expected.colors.toml`, and what the resolver prints for that file, as
//! `expected.tsv`. A case the converter generates nothing for has neither.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};

use motif_core::omarchy::{
    OmarchyAlacritty, OmarchyColors, Resolved, colors_from_alacritty, read_alacritty, read_colors,
};
use motif_core::{LoadError, Source, SourceKind};

fn fixtures(group: &str) -> Vec<PathBuf> {
    fixtures_with(group, "colors.toml")
}

fn fixtures_with(group: &str, file: &str) -> Vec<PathBuf> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/omarchy")
        .join(group);
    let mut dirs: Vec<PathBuf> = fs::read_dir(root)
        .expect("fixture folder")
        .map(|entry| entry.expect("fixture entry").path())
        .filter(|path| path.join(file).is_file())
        .collect();
    dirs.sort();
    dirs
}

fn check(dir: &Path) {
    let resolved = read_colors(&dir.join("colors.toml")).expect("colors.toml");
    check_resolved(dir, &resolved);
}

fn check_resolved(dir: &Path, resolved: &Resolved) {
    let expected = fs::read_to_string(dir.join("expected.tsv")).expect("expected.tsv");
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
    assert_ne!(cases.len(), 0, "no edge-case fixtures");
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
        assert_ne!(theme.name, "");
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

#[test]
fn alacritty_cases_convert_like_omarchy() {
    let cases = fixtures_with("alacritty", "alacritty.toml");
    let mut converted = 0;
    for dir in &cases {
        let alacritty = dir.join("alacritty.toml");
        let bytes = fs::read(&alacritty).expect("alacritty.toml");
        let actual = colors_from_alacritty(&bytes);
        let loaded = OmarchyAlacritty.load(dir);
        if let Ok(expected) = fs::read(dir.join("expected.colors.toml")) {
            converted += 1;
            assert_eq!(actual.as_deref(), Ok(&expected[..]), "{}", dir.display());
            check_resolved(dir, &read_alacritty(&alacritty).unwrap());
            let theme = loaded.unwrap().theme;
            assert_eq!(theme.provenance.kind, SourceKind::OmarchyAlacritty);
        } else {
            assert!(actual.is_err(), "{}", dir.display());
            assert!(
                matches!(loaded, Err(LoadError::OmarchyAlacritty { .. })),
                "{}",
                dir.display()
            );
        }
    }
    assert_ne!(converted, 0, "no converted alacritty fixtures");
    assert_ne!(converted, cases.len(), "no unconverted alacritty fixtures");
}
