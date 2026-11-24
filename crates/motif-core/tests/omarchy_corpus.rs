//! Compares the Omarchy port with Omarchy's resolver over a folder of
//! third-party `colors.toml` files.
//!
//! Runs only when `MOTIF_OMARCHY_CORPUS` names a folder. Every file in it
//! named `colors.toml` or ending in `.colors.toml` is resolved by both motif
//! and the vendored `omarchy-theme-color`, which needs bash and gawk. Run it
//! with `--nocapture` to see the themes motif cannot turn into a theme.

use std::fmt::Write as _;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use motif_core::omarchy::read_colors;
use motif_core::{Provenance, SourceKind};

#[test]
fn corpus_resolves_like_omarchy() {
    let Some(corpus) = std::env::var_os("MOTIF_OMARCHY_CORPUS") else {
        return;
    };
    let tool = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/omarchy/tools/omarchy-theme-color");

    let mut files: Vec<PathBuf> = fs::read_dir(&corpus)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name == "colors.toml" || name.ends_with(".colors.toml"))
        })
        .collect();
    files.sort();
    assert!(!files.is_empty(), "no colors.toml files in the corpus");

    let mut disagreements = Vec::new();
    let mut unconverted = Vec::new();
    for file in &files {
        let output = Command::new("bash")
            .arg(&tool)
            .arg("--file")
            .arg(file)
            .arg("--all")
            .env("LC_ALL", "C.UTF-8")
            .output()
            .unwrap();
        assert!(output.status.success(), "{}", file.display());
        let expected = String::from_utf8(output.stdout).unwrap();

        let resolved = read_colors(file).unwrap();
        let actual = resolved
            .iter()
            .fold(String::new(), |mut out, (key, value)| {
                let _ = writeln!(out, "{key}\t{value}");
                out
            });
        if actual != expected {
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
