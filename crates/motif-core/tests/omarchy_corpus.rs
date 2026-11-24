//! Compares the Omarchy port with Omarchy's scripts over a folder of
//! third-party theme files.
//!
//! Runs only when `MOTIF_OMARCHY_CORPUS` names a folder. Every file in it
//! named `colors.toml` or ending in `.colors.toml` is resolved by both motif
//! and the vendored `omarchy-theme-color`. Every file ending in
//! `.alacritty.toml` is converted by both motif and the vendored
//! `omarchy-theme-colors-from-alacritty`, and the two results are resolved as
//! above; a `NAME.light.mode` file beside `NAME.alacritty.toml` stands for a
//! `light.mode` in that theme. The scripts need bash and gawk. Run it with
//! `--nocapture` to see the themes motif cannot turn into a theme.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use motif_core::omarchy::{
    OmarchyAlacritty, Resolved, colors_from_alacritty, read_alacritty, read_colors,
};
use motif_core::{Provenance, Source, SourceKind};

fn tool(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/omarchy/tools")
        .join(name)
}

/// Returns the files in `corpus` whose names `wanted` accepts, sorted.
fn corpus_files(corpus: &Path, wanted: impl Fn(&str) -> bool) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = fs::read_dir(corpus)
        .expect("corpus folder")
        .map(|entry| entry.expect("corpus entry").path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(&wanted)
        })
        .collect();
    files.sort();
    files
}

/// Runs the vendored resolver over a `colors.toml`.
fn omarchy_resolve(file: &Path) -> String {
    let output = Command::new("bash")
        .arg(tool("omarchy-theme-color"))
        .arg("--file")
        .arg(file)
        .arg("--all")
        .env("LC_ALL", "C.UTF-8")
        .output()
        .expect("bash");
    assert!(output.status.success(), "{}", file.display());
    String::from_utf8(output.stdout).expect("UTF-8 output")
}

/// Formats a palette as `omarchy-theme-color --all` prints it.
fn tsv(resolved: &Resolved) -> String {
    resolved
        .iter()
        .fold(String::new(), |mut out, (key, value)| {
            let _ = writeln!(out, "{key}\t{value}");
            out
        })
}

#[test]
fn corpus_resolves_like_omarchy() {
    let Some(corpus) = std::env::var_os("MOTIF_OMARCHY_CORPUS") else {
        return;
    };
    let files = corpus_files(Path::new(&corpus), |name| {
        name == "colors.toml" || name.ends_with(".colors.toml")
    });
    assert!(!files.is_empty(), "no colors.toml files in the corpus");

    let mut disagreements = Vec::new();
    let mut unconverted = Vec::new();
    for file in &files {
        let expected = omarchy_resolve(file);
        let resolved = read_colors(file).unwrap();
        if tsv(&resolved) != expected {
            disagreements.push(file.display().to_string());
        }

        let provenance = Provenance::local(SourceKind::OmarchyColors, file);
        if let Err(err) = resolved.to_theme(String::new(), provenance) {
            let cause = std::error::Error::source(&err)
                .map(|cause| format!(": {cause}"))
                .unwrap_or_default();
            unconverted.push(format!("{}: {err}{cause}", file.display()));
        }
    }

    println!(
        "{} files, {} disagreements, {} not convertible",
        files.len(),
        disagreements.len(),
        unconverted.len()
    );
    for line in &unconverted {
        println!("not convertible: {line}");
    }
    assert!(
        disagreements.is_empty(),
        "motif disagrees with Omarchy on:\n{}",
        disagreements.join("\n")
    );
}

#[test]
fn alacritty_corpus_converts_like_omarchy() {
    let Some(corpus) = std::env::var_os("MOTIF_OMARCHY_CORPUS") else {
        return;
    };
    let files = corpus_files(Path::new(&corpus), |name| name.ends_with(".alacritty.toml"));
    assert!(!files.is_empty(), "no alacritty.toml files in the corpus");

    let scratch = tempfile::TempDir::new().unwrap();
    let mut disagreements = Vec::new();
    let mut unconverted = Vec::new();
    let mut light = 0;
    for (i, file) in files.iter().enumerate() {
        // The converter writes into the theme folder, so it runs on a copy.
        let theme = scratch.path().join(i.to_string());
        fs::create_dir(&theme).unwrap();
        let bytes = fs::read(file).unwrap();
        fs::write(theme.join("alacritty.toml"), &bytes).unwrap();
        let marker = file
            .to_string_lossy()
            .replace(".alacritty.toml", ".light.mode");
        if Path::new(&marker).is_file() {
            fs::write(theme.join("light.mode"), "").unwrap();
            light += 1;
        }

        let status = Command::new("bash")
            .arg(tool("omarchy-theme-colors-from-alacritty"))
            .arg(&theme)
            .env("LC_ALL", "C.UTF-8")
            .stderr(std::process::Stdio::null())
            .status()
            .unwrap();
        assert!(status.success(), "{}", file.display());
        let expected = fs::read(theme.join("colors.toml")).ok();
        let actual = colors_from_alacritty(&bytes).ok();
        if actual != expected {
            disagreements.push(format!("{}: generated colors.toml", file.display()));
            continue;
        }
        if expected.is_none() {
            unconverted.push(format!(
                "{}: Omarchy generates no colors.toml",
                file.display()
            ));
            continue;
        }

        // Read the copy, whose generated colors.toml motif would prefer, as
        // motif reads a legacy theme: through alacritty.toml.
        let resolved = read_alacritty(&theme.join("alacritty.toml")).unwrap();
        if tsv(&resolved) != omarchy_resolve(&theme.join("colors.toml")) {
            disagreements.push(format!("{}: resolved palette", file.display()));
        }
        match OmarchyAlacritty.load(&theme) {
            Ok(loaded) => assert_eq!(loaded.theme.provenance.kind, SourceKind::OmarchyAlacritty),
            Err(err) => {
                let cause = std::error::Error::source(&err)
                    .map(|cause| format!(": {cause}"))
                    .unwrap_or_default();
                unconverted.push(format!("{}: {err}{cause}", file.display()));
            }
        }
    }

    println!(
        "{} alacritty.toml files ({light} with light.mode), {} disagreements, {} not convertible",
        files.len(),
        disagreements.len(),
        unconverted.len()
    );
    for line in &unconverted {
        println!("not convertible: {line}");
    }
    assert!(
        disagreements.is_empty(),
        "motif disagrees with Omarchy on:\n{}",
        disagreements.join("\n")
    );
}
