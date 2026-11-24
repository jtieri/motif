//! Checks the palettes made from Omarchy's built-in themes.
//!
//! Each palette is compared with the file under `fixtures/palettes`, so that
//! any change to the output shows up in review. Run with `MOTIF_BLESS=1` to
//! rewrite the files after an intended change.

use std::fs;
use std::path::{Path, PathBuf};

use motif_core::omarchy::OmarchyColors;
use motif_core::{Mode, Options, Palette, Semantic, Source};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn built_ins() -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = fs::read_dir(fixtures().join("omarchy/themes"))
        .expect("themes folder")
        .map(|entry| entry.expect("theme entry").path())
        .collect();
    dirs.sort();
    dirs
}

/// Maps the built-in theme in `dir`.
fn palette(dir: &Path, semantic: Semantic) -> Palette {
    let theme = OmarchyColors.load(dir).expect("theme loads").theme;
    let options = Options {
        semantic,
        ..Options::default()
    };
    Palette::from_theme(&theme, &options).expect("palette")
}

#[test]
fn built_in_themes_match_their_snapshots() {
    let bless = std::env::var_os("MOTIF_BLESS").is_some();
    let dirs = built_ins();
    assert_eq!(dirs.len(), 23);
    for dir in &dirs {
        let palette = palette(dir, Semantic::Literal);
        let report = palette.validate();
        assert_eq!(report.problems, [], "{}", dir.display());

        let json = palette.to_json();
        let name = dir.file_name().expect("folder name").to_string_lossy();
        let snapshot = fixtures().join("palettes").join(format!("{name}.json"));
        if bless {
            fs::create_dir_all(snapshot.parent().expect("parent")).expect("create folder");
            fs::write(&snapshot, &json).expect("write snapshot");
        } else {
            let expected = fs::read_to_string(&snapshot).expect("snapshot; run with MOTIF_BLESS=1");
            assert_eq!(json, expected, "{}", snapshot.display());
        }
    }
}

#[test]
fn every_semantic_choice_gives_a_valid_palette() {
    for dir in built_ins() {
        for semantic in [Semantic::Literal, Semantic::Tinted, Semantic::Fixed] {
            let report = palette(&dir, semantic).validate();
            assert_eq!(report.problems, [], "{} {semantic}", dir.display());
        }
    }
}

#[test]
fn output_is_deterministic() {
    for dir in built_ins() {
        assert_eq!(
            palette(&dir, Semantic::Literal).to_json(),
            palette(&dir, Semantic::Literal).to_json()
        );
    }
}

#[test]
fn pins_identity_colors_from_the_theme() {
    let dir = fixtures().join("omarchy/themes/catppuccin");
    let palette = palette(&dir, Semantic::Literal);
    let role = |key| palette.palette.get(key).expect(key).to_string();
    assert_eq!(palette.meta.mode, Mode::Dark);
    assert_eq!(palette.meta.slug.as_str(), "catppuccin");
    assert_eq!(palette.meta.source_color.to_string(), "#89b4fa");
    assert_eq!(role("primary"), "#89b4fa");
    assert_eq!(role("accent"), "#89b4fa");
    assert_eq!(role("surface"), "#1e1e2e");
    assert_eq!(role("surface_container_lowest"), "#101019");
    assert_eq!(role("surface_container_low"), "#161622");
    assert_eq!(role("surface_container"), "#313244");
    assert_eq!(role("on_surface"), "#cdd6f4");
    assert_eq!(role("on_surface_variant"), "#bac2de");
    assert_eq!(role("on_surface_dim"), "#6c7086");
    assert_eq!(role("outline"), "#585b70");
    assert_eq!(role("error"), "#f38ba8");
    assert_eq!(role("warning"), "#f9e2af");
    assert_eq!(role("success"), "#a6e3a1");
    // Catppuccin Mocha's curated palette pins these tones for its blue and
    // red, from the same matugen recipe.
    assert_eq!(role("on_primary"), "#06305f");
    assert_eq!(role("primary_container"), "#254777");
    assert_eq!(role("on_error"), "#551d2f");
    assert_eq!(role("error_container"), "#703345");
    // Omarchy's Catppuccin sets no cursor, so Omarchy uses the foreground.
    assert_eq!(
        palette.ansi.get("cursor").expect("cursor").to_string(),
        "#cdd6f4"
    );
    assert_eq!(palette.meta.accents.available[0], "red");
}

#[test]
fn records_the_source_without_a_local_path() {
    let dir = fixtures().join("omarchy/themes/nord");
    let mut theme = OmarchyColors.load(&dir).expect("theme").theme;
    assert!(theme.provenance.path.is_some());
    let json = |theme: &motif_core::Theme| {
        let palette = Palette::from_theme(theme, &Options::default()).expect("palette");
        serde_json::to_value(&palette).expect("JSON")["meta"]["source"].clone()
    };
    assert_eq!(
        json(&theme),
        serde_json::json!({ "kind": "omarchy-colors" })
    );
    theme.provenance.url = Some("https://github.com/example/nord".into());
    theme.provenance.commit = Some("4574aa8".into());
    assert_eq!(
        json(&theme),
        serde_json::json!({
            "kind": "omarchy-colors",
            "url": "https://github.com/example/nord",
            "commit": "4574aa8",
        })
    );
}

#[test]
fn overrides_name_and_slug() {
    let dir = fixtures().join("omarchy/themes/tokyo-night");
    let theme = OmarchyColors.load(&dir).expect("theme").theme;
    let options = Options {
        name: Some("Night City".into()),
        ..Options::default()
    };
    let palette = Palette::from_theme(&theme, &options).expect("palette");
    assert_eq!(palette.meta.name, "Night City");
    assert_eq!(palette.meta.slug.as_str(), "night-city");

    let options = Options {
        slug: Some("tn".parse().expect("slug")),
        ..Options::default()
    };
    let palette = Palette::from_theme(&theme, &options).expect("palette");
    assert_eq!(palette.meta.name, "Tokyo Night");
    assert_eq!(palette.meta.slug.as_str(), "tn");

    let options = Options {
        name: Some("東京".into()),
        ..Options::default()
    };
    assert!(Palette::from_theme(&theme, &options).is_err());
}

/// Validates the dotfiles' own palettes, when `MOTIF_DOTFILES_PALETTES`
/// names the folder that holds them.
#[test]
fn curated_palettes_pass_validation() {
    let Some(dir) = std::env::var_os("MOTIF_DOTFILES_PALETTES") else {
        return;
    };
    let mut files: Vec<PathBuf> = fs::read_dir(dir)
        .expect("palette folder")
        .map(|entry| entry.expect("palette entry").path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "json"))
        .collect();
    files.sort();
    assert_ne!(files, Vec::<PathBuf>::new(), "no palettes in the folder");
    for file in &files {
        let text = fs::read_to_string(file).expect("palette");
        let doc: serde_json::Value = serde_json::from_str(&text).expect("JSON");
        let report = motif_core::validate::validate(&doc);
        assert_eq!(report.problems, [], "{}", file.display());
        println!(
            "{}: valid, {} contrast warnings",
            file.display(),
            report.warnings.len()
        );
        for warning in &report.warnings {
            println!("  {warning}");
        }
    }
}
